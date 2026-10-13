//! Reads the transport's own vocabulary at build time.
//!
//! `harness::tracker` turns a transport `reason` into a stable `&'static str`
//! code, and a code that is not in its list collapses to `transport-refused` —
//! which is the exact defect the whole outcome taxonomy exists to prevent,
//! committed one level down. A hand-maintained copy of that vocabulary had 24
//! entries when the transport had **69**, so fifty-three distinguishable
//! refusals arrived as one.
//!
//! Two lists cannot disagree if there is only one. This extracts the reasons
//! from `src/transport` and writes them out as an array the crate includes, so
//! a refusal added to the transport re-derives the vocabulary and no drift is
//! possible.
//!
//! It read the vendored `skill/scripts/github.py` as well, while that file was
//! the implementation that ran. It is gone — this repository holds one
//! transport, in one language — and a reason the port does not have is a reason
//! nothing can answer with.
//!
//! Deterministic and offline: it reads a file that ships in the repository.

// A build script has no caller to hand a `Result` to: failing loudly at build
// time is the correct behaviour, and it is the one place in this crate where a
// panic is the honest answer rather than a backtrace where a refusal belonged.
#![allow(clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::Path;
use std::{env, fs};

#[path = "src/source_text.rs"]
mod source_text;

/// Run a git query rooted at `root`, answering `None` for anything short of a
/// clean, non-empty success — the command missing, the directory not a
/// repository, or a reply of only whitespace are all "cannot tell", not an
/// empty string standing in for one.
fn git_query(root: &str, args: &[&str]) -> Option<String> {
    let Ok(output) = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
    else {
        return None;
    };
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_owned())
}

/// Watch the git files whose change means this build's identity is stale: the
/// ref `HEAD` names, and the file that ref itself lives in.
///
/// Emitting `cargo::rerun-if-changed` at all — for `build.rs` and the ported
/// transport files, both below — disables Cargo's default "rerun if anything
/// in the package changed". Unless something also watches git state, a
/// commit or a branch switch elsewhere leaves `ESTIGIA_COMMIT` and
/// `ESTIGIA_DIRTY` at whatever this script last computed, replayed from
/// Cargo's build-script cache on every later `cargo build`. Measured, not
/// inferred: editing a file this walk does not watch and
/// rebuilding kept reporting the base commit as `dirty`.
///
/// `HEAD` alone is not enough: on a branch, `HEAD` is a symbolic ref and does
/// not change when a commit is made on that branch — only the ref it names,
/// e.g. `refs/heads/main`, does. And a worktree's `.git` is a file naming a
/// per-worktree git directory that holds `HEAD`, while branch refs live in
/// the *common* directory shared across worktrees. `git rev-parse --git-dir`
/// and `--git-common-dir` give the two, already resolved through the
/// worktree indirection, so this does not parse `.git` itself.
///
/// `packed-refs` is watched unconditionally alongside the loose ref file: a
/// `git gc` can move a branch's ref into it at any time, and — since Cargo
/// reruns a build script on *every* build until a watched path exists at all —
/// watching it before it is ever touched costs nothing but a rerun on every
/// build in that window, not the one-time cost a first read suggests.
///
/// Deliberately does not attempt to watch anything that would tell an edit to
/// an already-tracked file's *content* apart from a clean tree: see
/// `docs/honesty.md` for why that half of the identity can still go stale
/// after this.
fn watch_git_identity(root: &str) {
    let Some(git_dir) = git_query(root, &["rev-parse", "--git-dir"]) else {
        return;
    };
    let Some(common_dir) = git_query(root, &["rev-parse", "--git-common-dir"]) else {
        return;
    };
    let resolve = |raw: &str| -> std::path::PathBuf {
        let path = Path::new(raw);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            Path::new(root).join(path)
        }
    };
    let git_dir = resolve(&git_dir);
    let common_dir = resolve(&common_dir);

    println!("cargo::rerun-if-changed={}", git_dir.join("HEAD").display());
    if let Some(target) = git_query(root, &["symbolic-ref", "-q", "HEAD"]) {
        println!(
            "cargo::rerun-if-changed={}",
            common_dir.join(target).display()
        );
    }
    println!(
        "cargo::rerun-if-changed={}",
        common_dir.join("packed-refs").display()
    );
}

/// Capture the checkout's identity so two builds of the same crate version can
/// be told apart.
///
/// An installed binary carries nothing that says which
/// source it was built from, so `estigia --version` reports `0.1.0` for every
/// build ever made and an agent that suspects the gate is older than the code
/// it is reading cannot establish or rule that out. The settled word comes
/// later in `doctor`; this is the half that makes the word possible.
///
/// Best-effort and never fatal: a published crate or a machine without git (or
/// without this being a checkout) emits empty values and builds anyway. The
/// identity those builds report is exactly the unknown `doctor` must then
/// swallow as *cannot tell*, which is part of why the fallback is an explicit
/// `unknown` rather than a silent empty.
fn capture_identity(root: &str) {
    let git_match = |args: &[&str]| git_query(root, args);

    let commit = git_match(&["rev-parse", "HEAD"]);
    let short = git_match(&["rev-parse", "--short=10", "HEAD"]);
    // `--untracked-files=no`, deliberately: the question this flag answers is
    // whether the *tracked* tree this binary was compiled from still matches
    // the commit it names, not whether the working directory holds anything
    // extra. Plain `--porcelain` counts a stray scratch file the same as an
    // edit to a tracked source file, so a machine that always has one — a
    // build artifact outside `target/`, an editor swap file — reports `dirty`
    // forever and the row this feeds is permanently `skipped` on it, which is
    // the issue's own machine. An untracked file does not change the
    // *committed* tree this identity names, so it should not cost the row its
    // answer — a narrower claim than "compiled from": `rust_sources`, below,
    // walks the filesystem rather than git, so an untracked `.rs` file under
    // `src/transport` is folded into `TRANSPORT_VOCABULARY` while this flag
    // still reports the tree clean.
    // `git_query` already answers `None` for a reply of only whitespace, so
    // dirtiness is simply whether there was a porcelain line to read at all —
    // an `.is_some_and(|p| !p.is_empty())` here would be checking a case
    // `git_query` has already ruled out.
    let dirty = git_match(&["status", "--porcelain", "--untracked-files=no"]).is_some();

    // The identity a person reads. Always set, so `env!` in the crate can rely
    // on it; the value is `unknown` rather than empty when git could not answer.
    let version = format!(
        "{} ({}{})",
        env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION"),
        short.as_deref().unwrap_or("unknown build"),
        if dirty { ", dirty" } else { "" },
    );

    // `cargo::rustc-env` must be emitted before cargo writes the fingerprint of
    // this build script's outputs; it is, since this runs inside `main`.
    println!("cargo::rustc-env=ESTIGIA_VERSION={version}");
    println!(
        "cargo::rustc-env=ESTIGIA_COMMIT={}",
        commit.unwrap_or_default()
    );
    println!(
        "cargo::rustc-env=ESTIGIA_DIRTY={}",
        if dirty { "1" } else { "0" }
    );
}

/// The transport, which answers in this process.
const PORT: &str = "src/transport";

/// Every `.rs` file under a directory.
fn rust_sources(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs")
                && path.file_name().is_some_and(|name| name != "tests.rs")
            {
                found.push(path);
            }
        }
    }
    found
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let root = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    watch_git_identity(&root);
    capture_identity(&root);

    let mut reasons: BTreeSet<String> = BTreeSet::new();
    let mut with_action: BTreeSet<String> = BTreeSet::new();

    // The port answers `missing-argument`, `unknown-operation` and four more
    // the original never had — it does its own argument handling where the
    // Python had `argparse` — and every one of them reached a caller as
    // `transport-refused`, which is the defect stated at the top of this file,
    // committed again one implementation over.
    //
    // Measured through the server: `start_branch` refused with
    // `missing-argument` in the envelope and `(transport-refused)` on the
    // screen, and the operator was sent to `estigia config list`.
    //
    // The same extractor over both, for the reason the header gives: two lists
    // cannot disagree if there is only one.
    let mut ported = 0;
    for file in rust_sources(Path::new(PORT)) {
        println!("cargo::rerun-if-changed={}", file.display());
        let source = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", file.display()));
        // Stripped before anything slices it, so a comment cannot move a
        // boundary a later slice takes on raw text — the trap `AGENTS.md`
        // names by that name, and the reason `shipped_part`, `extract_reasons`
        // and `extract_constructed` all walk this rather than `source`.
        let stripped = source_text::code_of(&source);
        // Tests build whatever envelope they need, and a fixture's reason is not
        // one the product can answer with. So the file is cut where its inline
        // test module starts — and *only* there.
        //
        // It used to cut at the first `#[cfg(test)]` of any kind. In
        // `transport/mod.rs` that attribute is on `mod tests;` at line 47, a
        // one-line declaration of a file this walk already skips, so the cut
        // threw away the other nine tenths of the module. `Failure::envelope`
        // lives in there, and with it `read-failed`, `write-failed` and
        // `ambiguous-write` — three answers whose whole purpose is to be told
        // apart, arriving at the harness as one `transport-refused`.
        let shipped = source_text::shipped_part(&stripped);
        assert_reasons_covered(shipped, &file);
        with_action.extend(reasons_naming_a_continuation(shipped));
        for reason in extract_reasons(shipped)
            .into_iter()
            .chain(extract_constructed(shipped))
        {
            if reasons.insert(reason) {
                ported += 1;
            }
        }
    }
    // A port that contributes nothing is a walk that stopped finding files.
    assert!(
        ported > 0,
        "the port added no reasons of its own; the walk over {PORT} is broken"
    );

    assert_every_refusal_names_a_way_out(&reasons, &with_action);

    let mut rendered = String::from(
        "// Generated by build.rs from the transport source. Do not edit.\n\
         /// Every `reason` the transport can answer with, as stable codes.\n\
         ///\n\
         /// Derived from `src/transport` at build time so the vocabulary\n\
         /// and the refusals cannot drift. See `build.rs` for why that.\n\
         pub const TRANSPORT_VOCABULARY: &[&str] = &[\n",
    );
    for reason in &reasons {
        rendered.push_str(&format!("    {reason:?},\n"));
    }
    rendered.push_str("];\n");

    let out = Path::new(&env::var("OUT_DIR").expect("cargo sets OUT_DIR"))
        .join("transport_vocabulary.rs");
    fs::write(&out, rendered).expect("write the generated vocabulary");

    write_template_headings();
}

const TEMPLATE: &str = "skill/assets/analyst-issue-template.md";

fn template_headings(template: &str) -> Vec<String> {
    let mut inside = false;
    let mut headings = Vec::new();
    for line in template.lines() {
        if !inside {
            inside = line.trim_end() == "```markdown";
        } else if line.trim_end() == "```" {
            break;
        } else if line.starts_with("## ") {
            headings.push(line.trim_end().to_owned());
        }
    }
    headings
}

fn write_template_headings() {
    println!("cargo::rerun-if-changed={TEMPLATE}");
    let template = fs::read_to_string(TEMPLATE)
        .unwrap_or_else(|error| panic!("{TEMPLATE} must be readable: {error}"));
    let headings = template_headings(&template);
    assert!(
        headings.len() >= 2,
        "{TEMPLATE} yields {} headings; its fenced `markdown` block has moved or changed shape",
        headings.len()
    );
    let mut rendered = String::from(
        "/// The `##` headings of the analyst issue template, in template order.\n\
         pub const TEMPLATE_HEADINGS: &[&str] = &[\n",
    );
    for heading in &headings {
        rendered.push_str(&format!("    {heading:?},\n"));
    }
    rendered.push_str("];\n");
    let out =
        Path::new(&env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("template_headings.rs");
    fs::write(&out, rendered).expect("write the generated template headings");
}

/// The two shapes a `"reason":` value is allowed to name something other than
/// a literal: a call literally spelled `stop(` or `refuse(`, whose first
/// argument `extract_constructed` below already reads, or `Failure::envelope`'s
/// own match, which the same function scans in full.
const REASON_PARAMETER_SCOPES: [&str; 4] =
    ["fn stop(", "let stop = |", "fn refuse(", "let refuse = |"];

/// The window `extract_constructed` reads out of `Failure::envelope`: the
/// anchor through the end of the function that builds the envelope's table,
/// which is its first four-space closing brace.
///
/// One derivation for both readers rather than a boundary restated at each,
/// because the guard that clears a non-literal `"reason":` here and the walk
/// that extracts the window's literals have to agree about where it ends. They
/// did not: a measurement put the guard's end at the next column-zero `fn`,
/// seventy-one lines past where the walk stopped reading, so a `"reason":`
/// between the two cleared the guard and reached no vocabulary.
fn envelope_window(source: &str) -> Option<std::ops::Range<usize>> {
    let at = source.find("pub fn envelope(")?;
    let rest = &source[at..];
    let end = rest
        .find(
            "
    }",
        )
        .unwrap_or(rest.len());
    Some(at..at + end)
}

/// Fails the build when a `"reason":` value in `source` is neither a string
/// literal nor bound inside one of the two shapes above.
///
/// `extract_reasons` below reads a literal and silently skips anything else,
/// trusting that a skipped value's own literals turn up elsewhere in the
/// file. This is what checks that trust instead of assuming it
/// found four sites the trust did not hold for: `operation_marker`'s two
/// `format!` calls, `start_branch` handing `checkout_identity`'s bare
/// `verdict` variable to the envelope in `branch.rs`, and an inline `if`/`else`
/// in `require_pr_matches`. The next
/// one is meant to be caught here rather than found by accident later, the
/// way those four were.
///
/// Deliberately does not learn a shape: it does not parse `format!`, a
/// `match`, or an `if`. Teaching it one would need to agree with every call
/// site the same way the vocabulary itself does, which is the "two lists"
/// defect this crate exists to avoid one level down. A `"reason":` value this
/// walk cannot clear is a value the vocabulary should not clear either — the
/// answer is to give it a literal, as `operation_marker`'s callers now do.
///
/// `source` arrives already stripped of comments — `main` strips once at the
/// read, before `shipped_part` — so a comment spelling `"reason":` followed by
/// prose cannot abort the build over a refusal that does not exist.
fn assert_reasons_covered(source: &str, file: &Path) {
    let envelope = envelope_window(source);
    let mut rest = source;
    let mut base = 0usize;
    while let Some(at) = rest.find("\"reason\":") {
        let pos = base + at;
        let after = &rest[at + "\"reason\":".len()..];
        base = pos + "\"reason\":".len();
        rest = after;

        let trimmed = after.trim_start();
        if trimmed.starts_with('"') {
            continue;
        }

        let identifier: String = trimmed
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let tail = &trimmed[identifier.len()..].trim_start();
        let bare = identifier == "reason" && (tail.starts_with(',') || tail.starts_with('}'));

        // A `stop`/`refuse` scope is left as soon as a fresh top-level `fn`
        // opens after it — each anchor is a one-line function or closure, so
        // crossing one means this `"reason":` belongs to whatever comes after
        // it, not to the anchor found.
        let still_in_scope = |anchor_end: usize| -> bool {
            let between = &source[anchor_end..pos];
            !between.contains("\nfn ") && !between.contains("\npub fn ")
        };

        let covered = envelope
            .as_ref()
            .is_some_and(|window| window.contains(&pos))
            || (bare
                && REASON_PARAMETER_SCOPES.iter().any(|anchor| {
                    source[..pos]
                        .rfind(anchor)
                        .is_some_and(|found| still_in_scope(found + anchor.len()))
                }));

        if !covered {
            let snippet: String = trimmed.chars().take(40).collect();
            panic!(
                "{}: \"reason\": {snippet:?}… is neither a string literal nor bound inside \
                 `envelope()` or a `stop`/`refuse` call's reason parameter — build.rs cannot see \
                 this refusal reason, and it will collapse to `transport-refused` at the harness. \
                 Give it a literal.",
                file.display()
            );
        }
    }
}

/// Every reason the transport hands to a refusal constructor.
///
/// The port does not always spell the pair. `claim.rs` has `stop(reason, …)`,
/// and thirty-one of the codes it answers with reach a caller only through it —
/// so the generated vocabulary had them all missing, and every one of them
/// collapsed to `transport-refused` in the harness. That is the defect at the
/// top of this file, and it survived the change that was supposed to end it,
/// because the reference implementation happened to spell the pair literally
/// and the extractor was written against that spelling rather than against the
/// idea.
///
/// Matched on the call, not on any quoted kebab-case word: a literal argument
/// to something *named* `stop` is a refusal code, and a branch name that
/// happens to look like one is not passed to it.
fn extract_constructed(source: &str) -> BTreeSet<String> {
    let mut reasons = BTreeSet::new();
    // The envelope's own table. `Failure::Read`, `Timeout`, `Write` and their
    // neighbours do not construct a payload at all — they name their reason in
    // a `match self` that builds `(reason, detail, action)`, which is a third
    // spelling and was the third one missed. `read-failed` is in it, and a
    // `read-failed` that reaches the harness as `transport-refused` tells a run
    // to stop where the answer was *retry the read*.
    if let Some(window) = envelope_window(source) {
        let body = &source[window];
        let mut cursor = body;
        while let Some(quote) = cursor.find('"') {
            cursor = &cursor[quote + 1..];
            let Some(end) = cursor.find('"') else { break };
            let word = &cursor[..end];
            if word.contains('-')
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                reasons.insert(word.to_owned());
            }
            cursor = &cursor[end + 1..];
        }
    }
    for call in ["stop(", "refuse("] {
        let mut rest = source;
        while let Some(at) = rest.find(call) {
            rest = &rest[at + call.len()..];
            // The whole first argument, not the character after the bracket.
            //
            // `rustfmt` breaks a three-argument call across lines, so the quote
            // is rarely against it — and an argument is not always a literal at
            // all. `first_argument` says which shapes those are and why a comma
            // search is the wrong reader for them; this call site needs only to
            // know that it is handed the whole argument rather than whatever
            // precedes the first comma.
            //
            // What that cost, once: `cmd_reclaim`'s refusal was
            // `stop(if before.stale.is_empty() { "nothing-to-reclaim" } else {
            // "stale-self-requires-claim" }, …)`, two of the five reclaim
            // refusals whose own documentation said the distinctions between
            // them *are the point*, each sending the caller somewhere
            // different. Both were missing from the vocabulary and both reached
            // the harness as one `transport-refused`, while their sibling
            // `holder-not-stale` — written as a plain literal three lines away —
            // came through intact. That `if` was split into
            // `plan_takeover`'s own two plain-literal `stop` calls, so no reason
            // in the tree reaches this walk through a conditional's arms today.
            // The reader stays because that shape left the code and not the
            // language, and the walk is what would catch it coming back.
            for reason in literals_in(first_argument(rest)) {
                reasons.insert(reason);
            }
        }
    }
    reasons
}

/// The text of a call's first argument, from just after the opening bracket.
///
/// Bracket-counting rather than a comma search: the argument can be a `match`,
/// an `if`, a `format!` or a tuple, and every one of those holds commas of its
/// own that mean nothing to the caller.
fn first_argument(rest: &str) -> &str {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (at, c) in rest.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return &rest[..at],
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return &rest[..at],
            _ => {}
        }
    }
    rest
}

/// Every kebab-case string literal in a fragment of source.
///
/// The shape a reason has, and one no prose in an argument shares: a `detail`
/// is a sentence and an `action` is an instruction, both full of spaces.
fn literals_in(fragment: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = fragment;
    while let Some(at) = rest.find('"') {
        rest = &rest[at + 1..];
        let Some(end) = rest.find('"') else { break };
        let word = &rest[..end];
        if word.contains('-')
            && word
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            found.insert(word.to_owned());
        }
        rest = &rest[end + 1..];
    }
    found
}

/// Every `"reason": "..."` the transport can emit.
///
/// Matched on the literal pair rather than on any quoted lowercase word: the
/// transport is thousands of lines of shell-outs and prose, and a looser
/// pattern would sweep up branch names, label names and error text as though
/// they were codes.
fn extract_reasons(source: &str) -> BTreeSet<String> {
    let mut reasons = BTreeSet::new();
    let mut rest = source;
    while let Some(at) = rest.find("\"reason\":") {
        rest = &rest[at + "\"reason\":".len()..];
        let trimmed = rest.trim_start();
        let Some(body) = trimmed.strip_prefix('"') else {
            // `"reason": reason` — a variable, whose values are literals
            // elsewhere in the file and are picked up there.
            continue;
        };
        let Some(end) = body.find('"') else { break };
        let reason = &body[..end];
        if !reason.is_empty()
            && reason
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            reasons.insert(reason.to_owned());
        }
    }
    reasons
}

/// The frozen refusals, and the count the walk's floor is derived from.
const BASELINE: &str = "refusal-baseline.txt";

/// The three words a refusal may use to declare that no runnable continuation
/// exists. This is `src/outcome.rs`'s `NoCommandReason` closed vocabulary,
/// spelled here because `build.rs` is a separate crate that cannot import the
/// library; `tests/guards.rs` crosses the two so they cannot drift apart.
const NO_CONTINUATION_SHAPES: [&str; 3] = ["operator-knowledge", "world-action", "human-authority"];

/// Refusals that name no runnable continuation **on purpose**, each with the
/// shape of why, from `NO_CONTINUATION_SHAPES`.
///
/// A declaration here says the way out is not a command: the caller supplies a
/// value, the world must change, or a person holds the authority. The guard
/// refuses a declaration for a reason the transport already answers with an
/// action, so this table cannot hide a working continuation.
const BY_DESIGN: &[(&str, &str)] = &[
    ("blank-argument", "operator-knowledge"),
    ("invalid-horizon", "operator-knowledge"),
    ("invalid-operation-id", "operator-knowledge"),
];

/// The text of the `json!({ ... })` object a byte position sits inside, if it
/// sits inside one — the envelope whose `"action"` key decides whether a
/// `"reason":` literal names a continuation.
fn enclosing_json_object(source: &str, pos: usize) -> Option<&str> {
    let mut search_end = pos;
    while let Some(start) = source[..search_end].rfind("json!(") {
        let open = start + "json!".len();
        let argument = first_argument(&source[open + 1..]);
        let end = open + 1 + argument.len();
        if open < pos && pos < end {
            return Some(argument);
        }
        search_end = start;
    }
    None
}

/// The `"reason": "..."` literal a position opens, or `None` for a variable or
/// a value outside the code's kebab-case shape.
fn literal_reason_at(source: &str, pos: usize) -> Option<String> {
    let after = &source[pos + "\"reason\":".len()..];
    let body = after.trim_start().strip_prefix('"')?;
    let end = body.find('"')?;
    let reason = &body[..end];
    (!reason.is_empty()
        && reason
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
    .then(|| reason.to_owned())
}

/// How many top-level arguments a call holds, from just after its opening
/// bracket. A `stop` call with three passes an action; the one-argument closure
/// `plan_takeover` binds of the same name passes none.
fn argument_count(rest: &str) -> usize {
    let mut count = 0;
    let mut cursor = rest;
    loop {
        let argument = first_argument(cursor);
        count += 1;
        cursor = &cursor[argument.len()..];
        match cursor.strip_prefix(',') {
            Some(after_comma) => cursor = after_comma,
            None => return count,
        }
    }
}

/// Every reason whose operator-facing envelope carries a non-empty `action`.
///
/// Three shapes carry one: the `Failure::envelope` match's own arms; a
/// `stop`/`refuse` call passing a third argument, which is the module helper's
/// action; and a `"reason":` literal inside a `json!` object that also holds
/// `"action"`. A `"reason":` bound to a variable is left to the literal's own
/// site, the same trust `assert_reasons_covered` checks.
fn reasons_naming_a_continuation(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    if let Some(window) = envelope_window(source) {
        found.extend(literals_in(&source[window]));
    }
    for call in ["stop(", "refuse("] {
        let mut rest = source;
        while let Some(at) = rest.find(call) {
            rest = &rest[at + call.len()..];
            if argument_count(rest) >= 3 {
                found.extend(literals_in(first_argument(rest)));
            }
        }
    }
    let mut rest = source;
    let mut base = 0usize;
    while let Some(at) = rest.find("\"reason\":") {
        let pos = base + at;
        rest = &rest[at + "\"reason\":".len()..];
        base = pos + "\"reason\":".len();
        if let Some(block) = enclosing_json_object(source, pos)
            && block.contains("\"action\"")
            && let Some(reason) = literal_reason_at(source, pos)
        {
            found.insert(reason);
        }
    }
    found
}

/// The baseline file, read as the frozen refusals and the population count the
/// floor is derived from.
struct Baseline {
    population: usize,
    reasons: BTreeSet<String>,
}

fn read_baseline() -> Baseline {
    println!("cargo::rerun-if-changed={BASELINE}");
    let text = fs::read_to_string(BASELINE)
        .unwrap_or_else(|error| panic!("{BASELINE} must be readable: {error}"));
    let mut population = None;
    let mut reasons = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(count) = line.strip_prefix("population ") {
            population = count.trim().parse::<usize>().ok();
        } else {
            reasons.insert(line.to_owned());
        }
    }
    Baseline {
        population: population.unwrap_or_else(|| {
            panic!("{BASELINE} must record the last known count as `population <n>`")
        }),
        reasons,
    }
}

/// Fails the build when a refusal names no runnable continuation and declares
/// no reason why, when a baseline entry matches no such refusal, and when the
/// walk's population drops below the count recorded in the baseline.
///
/// The population is every distinct reason the transport can answer with — the
/// same set the walk above extracts for `TRANSPORT_VOCABULARY`. A reason is the
/// operator-visible identity of a refusal, so it is the unit an operator can act
/// on. A reason is satisfied when its envelope carries an `action`, when
/// `BY_DESIGN` declares a shape from `NO_CONTINUATION_SHAPES`, or when
/// `BASELINE` freezes it. The baseline may only shrink: an entry that matches no
/// unsatisfied refusal is an error naming that entry, which is how *the refusal
/// was fixed* is told apart from *the refusal moved into a shape the walk cannot
/// see*.
///
/// **What this cannot do.** It reads the operator-facing text for the presence
/// of an `action`; it cannot tell a correct continuation from one that parses,
/// runs and does not help. A refusal whose action names a command that does not
/// clear the block passes here, and only a person hitting it can see that. It
/// also cannot tell a genuine `BY_DESIGN` from a fixable refusal someone
/// declared to silence the guard, and it does not reach a refusal whose reason
/// is built by a shape `extract_reasons`/`extract_constructed` do not read —
/// `assert_reasons_covered` is what closes that class, not this.
fn assert_every_refusal_names_a_way_out(
    reasons: &BTreeSet<String>,
    with_action: &BTreeSet<String>,
) {
    let baseline = read_baseline();
    assert!(
        reasons.len() >= baseline.population,
        "only {} reasons were found in {PORT}; the floor is {}, the last known count recorded in \
         {BASELINE}. A population that shrank is a walk that stopped finding files, or a removal \
         that took reasons with it — check `git diff` for which before lowering the recorded count, \
         and lower it no further than the removal actually accounts for",
        reasons.len(),
        baseline.population,
    );

    for (reason, shape) in BY_DESIGN {
        assert!(
            reasons.contains(*reason),
            "BY_DESIGN declares `{reason}`, which is not a reason the transport answers with"
        );
        assert!(
            !with_action.contains(*reason),
            "BY_DESIGN declares `{reason}` as having no way out, but its envelope already carries \
             an action — remove the declaration rather than hide the continuation"
        );
        assert!(
            NO_CONTINUATION_SHAPES.contains(shape),
            "`{reason}` is declared by-design with shape `{shape}`, which is not one of \
             {NO_CONTINUATION_SHAPES:?}. A shape invented for one refusal is one nobody can compare \
             against another"
        );
    }

    let declared: BTreeSet<&str> = BY_DESIGN.iter().map(|(reason, _)| *reason).collect();
    let mut unsatisfied: BTreeSet<&str> = reasons
        .iter()
        .filter(|reason| !with_action.contains(*reason) && !declared.contains(reason.as_str()))
        .map(String::as_str)
        .collect();
    for entry in &baseline.reasons {
        assert!(
            unsatisfied.remove(entry.as_str()),
            "{BASELINE} lists `{entry}`, which matches no refusal that names no way out. Either it \
             was fixed — given an action or a by-design shape — and its line must go, or the walk no \
             longer finds it; the list may only shrink, so a stale entry is an error and not a note"
        );
    }
    assert!(
        unsatisfied.is_empty(),
        "these refusals name no runnable continuation and declare no reason why: {unsatisfied:?}. \
         Give each one an `action` a caller can run, or a BY_DESIGN shape saying why it cannot have \
         one — do not add it to {BASELINE}, which is frozen debt and may only shrink"
    );
}
