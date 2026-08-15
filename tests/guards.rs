// An assertion that panics is the assertion working. See `tests/pipe.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Population declarations, bound to the code they describe.
//!
//! A guard is a rule about a **population**: the set of things it is supposed to
//! catch. Whether it catches them is a claim, and a claim written in a comment
//! decays the moment the code beneath it moves — silently, because a comment
//! never fails to compile.
//!
//! So each declaration is parsed out of the source with `syn`, bound to the
//! syntax node it sits on, and **fingerprinted together with that node**.
//! Changing either one reopens the claim: the fingerprint stops matching, this
//! test fails, and the author has to look at the declaration again before
//! recording the new one.
//!
//! ```text
//! guard:population <family> <too-tight|too-loose|fail-closed>: <population and boundary>
//! ```
//!
//! # The proof boundary
//!
//! This proves that every declaration is well formed, that it is attached to a
//! real item, and that neither the rule nor the code under it has changed since
//! somebody last affirmed the pair. It does **not** prove any declaration is
//! true, and it does not prove every guard that needs one has one — a guard
//! with no declaration is invisible to this test, exactly as it is to a reader.
//!
//! And it fingerprints **the item the declaration sits on**, which for most of
//! them is the list. A declaration on a list therefore holds the rule and the
//! population, and not the code that matches against them: `control-surface`
//! says *matched on path fragments* and sits on `CONTROL_SURFACE`, so changing
//! `is_control_surface` leaves this test green. Measured by changing it. The
//! rule half still bites — the doc text is hashed with the item — so a boundary
//! sentence cannot drift silently, only the code beneath one can.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The three ways a population can be wrong, and nothing else.
///
/// A closed vocabulary for the same reason the rejection reasons are one: a
/// disposition invented for one guard is a disposition nobody can compare
/// against another.
const DISPOSITIONS: &[&str] = &["too-tight", "too-loose", "fail-closed"];

/// Every Rust source in the crate, found rather than listed.
///
/// This was two hand-written lists: seven files that carry declarations and one
/// more that only mentions them. Nothing was unguarded — but nothing kept the
/// lists honest either, so a declaration written in an eighth file would have
/// been parsed by nobody, fingerprinted by nobody and affirmed by nobody, while
/// reading on the page exactly like the ones that were. A guard that silently
/// covers less than it appears to is the one people stop checking behind.
///
/// It is also the same defect this crate keeps finding: one end written by hand,
/// the other in code, and nothing crossing them. Here the other end is the
/// directory, so the fix is to read the directory.
fn sources() -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root().join("src")];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(root())
                    .expect("every source sits under the crate root");
                found.push(
                    relative
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                );
            }
        }
    }
    found.sort();
    assert!(
        found.len() > 20,
        "the crate has {} sources; the walk found too few to be reading the tree",
        found.len()
    );
    found
}

/// One declaration, and the node it is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    family: String,
    disposition: String,
    /// The population and its boundary, as written.
    text: String,
    /// The item this sits on, named for the failure message.
    item: String,
    /// A hash over the declaration **and** the item's tokens.
    fingerprint: u64,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FNV-1a. Small, dependency-free, and stable across runs — which is all a
/// fingerprint has to be. It is not a security primitive and nothing here
/// treats it as one.
fn fingerprint(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// One node's tokens, as text. Position-free by construction.
fn tokens<T: quote::ToTokens>(node: &T) -> String {
    node.to_token_stream().to_string()
}

/// The doc comment attached to an item, joined.
fn documentation(attributes: &[syn::Attribute]) -> String {
    attributes
        .iter()
        .filter_map(|attribute| {
            let syn::Meta::NameValue(named) = &attribute.meta else {
                return None;
            };
            if !named.path.is_ident("doc") {
                return None;
            }
            let syn::Expr::Lit(literal) = &named.value else {
                return None;
            };
            let syn::Lit::Str(text) = &literal.lit else {
                return None;
            };
            Some(text.value())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A short name for an item, for the message when its fingerprint moves.
fn name_of(item: &syn::Item) -> Option<(String, &[syn::Attribute])> {
    Some(match item {
        syn::Item::Const(node) => (format!("const {}", node.ident), node.attrs.as_slice()),
        syn::Item::Static(node) => (format!("static {}", node.ident), node.attrs.as_slice()),
        syn::Item::Fn(node) => (format!("fn {}", node.sig.ident), node.attrs.as_slice()),
        syn::Item::Enum(node) => (format!("enum {}", node.ident), node.attrs.as_slice()),
        syn::Item::Struct(node) => (format!("struct {}", node.ident), node.attrs.as_slice()),
        _ => return None,
    })
}

/// Every declaration in the crate, bound to its node.
fn declarations() -> Vec<Declaration> {
    let mut found = Vec::new();
    for relative in sources() {
        let path = root().join(&relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        let parsed = syn::parse_file(&source)
            .unwrap_or_else(|error| panic!("{} must parse: {error}", path.display()));

        for item in &parsed.items {
            let Some((name, attributes)) = name_of(item) else {
                continue;
            };
            let documentation = documentation(attributes);
            let lines: Vec<&str> = documentation.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                let Some(first) = line.trim().strip_prefix("guard:population ") else {
                    continue;
                };
                // A declaration is a paragraph, not a line. It has to name a
                // population *and* a boundary, and neither fits in eighty
                // columns — reading only the first line would judge every
                // declaration on its opening clause.
                let mut rest = first.to_owned();
                for continuation in lines.iter().skip(at + 1) {
                    let continuation = continuation.trim();
                    if continuation.is_empty() || continuation.starts_with("guard:population ") {
                        break;
                    }
                    rest.push(' ');
                    rest.push_str(continuation);
                }
                let rest = rest.as_str();
                let (head, text) = rest.split_once(':').unwrap_or((rest, ""));
                let mut words = head.split_whitespace();
                let family = words.next().unwrap_or_default().to_owned();
                let disposition = words.collect::<Vec<_>>().join(" ");

                // The item's own tokens, without the doc attributes — so the
                // declaration text and the code contribute separately and a
                // reader can tell which of the two moved.
                //
                // Tokens rather than `Debug`: `syn`'s `Debug` prints spans, and
                // a fingerprint taken from it moves when a comment *above* the
                // code is deleted. That is a claim reopened for a reason this
                // test's own documentation said would not reopen it — a guard
                // crying wolf teaches people to re-record without reading,
                // which is the only way to make this worthless. A token stream
                // carries no positions, so reformatting and comments are
                // invisible and a changed expression is not.
                let body = match item {
                    syn::Item::Const(node) => tokens(&node.expr),
                    syn::Item::Enum(node) => tokens(&node.variants),
                    syn::Item::Fn(node) => tokens(&node.block),
                    syn::Item::Struct(node) => tokens(&node.fields),
                    syn::Item::Static(node) => tokens(&node.expr),
                    // Not an empty body. Every kind above contributes the part
                    // a rule actually lives in, and a kind nobody taught this
                    // walker about would contribute nothing — so the claim
                    // would be fingerprinted over its prose alone, the code
                    // under it could be rewritten end to end, and this test
                    // would go on reporting that nothing had moved. That is the
                    // one failure the whole mechanism exists to prevent, and it
                    // would arrive silently, on the day somebody declares a
                    // population on an `impl`, a `trait`, a `type` or a `mod`.
                    //
                    // Latent rather than live when it was found: all eleven
                    // declarations sit on a `const` or a `fn` today. A trap set
                    // and not yet stepped on is still a trap.
                    other => panic!(
                        "`{family}` at {relative} declares a population on a kind this walker                          cannot read the body of ({}), so only its prose would be fingerprinted                          and a change to the rule itself would never reopen the claim. Teach the                          `body` match how to read that kind, or move the declaration onto the                          item that holds the rule.",
                        match other {
                            syn::Item::Impl(_) => "an impl block",
                            syn::Item::Trait(_) => "a trait",
                            syn::Item::Type(_) => "a type alias",
                            syn::Item::Mod(_) => "a module",
                            syn::Item::Macro(_) => "a macro",
                            _ => "an item this walker does not name",
                        }
                    ),
                };
                found.push(Declaration {
                    fingerprint: fingerprint(&format!(
                        "{relative}|{name}|{family}|{disposition}|{}|{body}",
                        text.trim()
                    )),
                    family,
                    disposition,
                    text: text.trim().to_owned(),
                    item: format!("{relative} :: {name}"),
                });
            }
        }
    }
    found
}

#[test]
#[ignore = "a reporter, not a check: run with --ignored to read every declaration"]
fn list_declarations() {
    for declaration in declarations() {
        println!(
            "(\"{}\", {}),  // {} [{}]
    {}
",
            declaration.family,
            declaration.fingerprint,
            declaration.item,
            declaration.disposition,
            declaration.text
        );
    }
}

#[test]
fn every_declaration_is_well_formed() {
    let declarations = declarations();
    assert!(
        declarations.len() >= 3,
        "no population declarations were found; the parser is broken or they were all removed"
    );

    for declaration in &declarations {
        assert!(
            !declaration.family.is_empty()
                && declaration
                    .family
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
            "{}: `{}` is not a kebab-case family name",
            declaration.item,
            declaration.family
        );
        assert!(
            DISPOSITIONS.contains(&declaration.disposition.as_str()),
            "{}: `{}` is not one of {DISPOSITIONS:?} — a disposition invented for one guard is \
             one nobody can compare against another",
            declaration.item,
            declaration.disposition
        );
        assert!(
            declaration.text.len() > 30,
            "{}: the declaration names no population and no boundary: {:?}",
            declaration.item,
            declaration.text
        );
    }
}

#[test]
fn no_declaration_is_invisible_to_this_test() {
    // The guard on the guard. `syn` reads doc attributes; a `guard:population`
    // written as a plain `//` comment inside a function body is discarded by
    // the parser and checked by nobody — which is worse than not writing one,
    // because it reads as though somebody is watching. One was, in
    // `harness::tracker::translate`, until this test was written.
    //
    // References to a family from a test are not declarations, so they are
    // matched separately and required to name a family that exists.
    let declared: Vec<String> = declarations()
        .into_iter()
        .map(|declaration| declaration.family)
        .collect();

    let mut written = 0;
    let mut referenced = Vec::new();
    let scanned = sources();
    for relative in &scanned {
        let source = std::fs::read_to_string(root().join(relative)).expect("a readable source");
        for line in source.lines() {
            let Some(rest) = line
                .trim()
                .trim_start_matches("///")
                .trim_start_matches("//")
                .trim()
                .strip_prefix("guard:population ")
            else {
                continue;
            };
            let family = rest.split_whitespace().next().unwrap_or_default();
            // A declaration names a disposition after the family; a reference
            // does not.
            let is_declaration = rest
                .split_whitespace()
                .nth(1)
                .is_some_and(|word| DISPOSITIONS.contains(&word.trim_end_matches(':')));
            if is_declaration {
                written += 1;
            } else {
                referenced.push((relative, family.trim_end_matches(['.', ',']).to_owned()));
            }
        }
    }

    assert_eq!(
        written,
        declared.len(),
        "{written} declarations are written in the source and {} were parsed. One is somewhere          `syn` cannot see it — a plain `//` comment inside a function body, most likely.",
        declared.len()
    );

    for (relative, family) in referenced {
        assert!(
            declared.contains(&family),
            "{relative} refers to the `{family}` population, which nothing declares"
        );
    }
}

#[test]
fn no_two_guards_claim_the_same_family() {
    // Two declarations under one name make the pair unreadable: a reader cannot
    // tell which population a failure belongs to.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for declaration in declarations() {
        if let Some(first) = seen.insert(declaration.family.clone(), declaration.item.clone()) {
            panic!(
                "`{}` is declared twice: {first} and {}",
                declaration.family, declaration.item
            );
        }
    }
}

#[test]
fn a_declaration_and_its_code_have_not_moved_since_they_were_affirmed() {
    // The mechanism the handoff asked for: the claim is frozen against the node
    // it describes, so editing either the rule or the code beneath it reopens
    // it. This is the only test in the crate whose failure means *go and read
    // something*, not *go and fix something* — and the fix is to look at the
    // declaration, decide it is still true, and record the new fingerprint.
    let recorded: BTreeMap<&str, u64> = AFFIRMED.iter().copied().collect();
    let current = declarations();

    for declaration in &current {
        let Some(expected) = recorded.get(declaration.family.as_str()) else {
            panic!(
                "`{}` at {} has no affirmed fingerprint. Read the declaration, decide whether it \
                 is still true, then add:\n    (\"{}\", {}),",
                declaration.family, declaration.item, declaration.family, declaration.fingerprint
            );
        };
        assert_eq!(
            declaration.fingerprint, *expected,
            "`{}` at {} changed. The rule, the code under it, or both have moved, so the claim \
             is open again. Read it, decide whether it still holds, then record:\n    (\"{}\", \
             {}),",
            declaration.family, declaration.item, declaration.family, declaration.fingerprint
        );
    }

    for (family, _) in AFFIRMED {
        assert!(
            current.iter().any(|found| found.family == *family),
            "`{family}` is affirmed and no longer declared anywhere; take it out of AFFIRMED so \
             the list keeps its grip"
        );
    }
}

/// The fingerprints somebody looked at and affirmed.
///
/// Not a cache and not a lockfile: a record that a person read the declaration
/// beside this code and decided it was still true. Regenerating it without
/// reading is the one way to make this test worthless, and no tooling here does
/// that for you.
const AFFIRMED: &[(&str, u64)] = &[
    // Affirmed 2026-07-31, by reading each one against the code beneath it.
    //
    // `write-tools` — reopened when the list stopped being Claude Code's alone.
    // It had been for four rounds, while gates went in for five more agents:
    // every one of those gates fired, reached the classifier, found a name it
    // did not know and stood aside. Now it carries Codex's `apply_patch`,
    // Gemini and Qwen's `write_file`/`replace`, and OpenCode's `patch`, matched
    // case-insensitively because `Edit` and `edit` are one tool. Still
    // too-tight, and still for the right reason.
    ("write-tools", 6860768011680559988),
    // `shell-tools` — split out of `write-tools` while reading it. These are the
    // tools whose *argument* decides what they are: `git status` through one is
    // a read and `git push` through the same one is a boundary, so they cannot
    // share a list with tools that are writes by construction.
    ("shell-tools", 3959669076258420723),
    // `irreversible-shell` — relabelled from fail-closed while reading it. A
    // spelling that is not on the list escapes, and escaping is not failing
    // closed. What is fail-closed is the substring *matching*, which is a
    // different claim and now says so.
    // Re-read on 2026-08-14 and still true, and narrower than it was. The match
    // now ends at a word: `contains` alone refused `git merge-base
    // --is-ancestor` as `git merge`, which is a read this crate's own transport
    // performs. The claim is unchanged — these spellings are gated, and they are
    // not the only spellings — while one class of false refusal is gone.
    ("irreversible-shell", 17043155859771761798),
    // `repository-shell` — the git commands that write. Same shape and same
    // limits as `write-tools`, and it exists so that composing `git worktree
    // add` by hand is gated rather than invisible.
    // Reopened 2026-08-01 by narrowing it. It had claimed "every spelling of a
    // repository write reachable from a shell" — a population it never covered,
    // since a redirect and a `sed -i` write the same tree without naming git.
    // Now it claims only the spellings that name git, which is what the list
    // holds. Read against the constant: still too-tight, and the escapes named
    // (an alias, a wrapper script) are the ones that remain.
    ("repository-shell", 9984610503202939079),
    // `exit-code` — genuinely fail-closed, and the only one that is. `0` to `5`
    // is the whole contract the transport implements, and the `_` arm sends
    // anything else to `Unknown`, never to the stop arm. Moved onto the item
    // while reading it: as a `//` comment inside the function body, `syn` never
    // saw it and no test had ever checked it.
    // Re-affirmed 2026-08-14: `1` is now read twice, on whether the transport
    // declared it had already written. The population is unchanged — still the
    // codes `0` through `5` — and so is the fail-closed boundary: a code nobody
    // has read still lands in `Unknown` rather than in the stop arm.
    //
    // Re-affirmed again the same day, after review: the second axis of that new
    // arm was wrong. It answered `ExactReplaySafe` beside an action reading *do
    // not bind review or CI to anything yet: re-read the pull request*, and
    // repeating the identical call mints a fresh epoch over a head somebody else
    // pushed. `StatusRequired` is the one that matches the sentence. The
    // population and the boundary are still untouched.
    ("exit-code", 6935824797548370733),
    // `writing-shell` — new on 2026-08-01, closing the hole `repository-shell`
    // used to claim and never covered. Read against the function: the four
    // shapes it names (redirect, in-place edit, inline code, the copy/move/
    // delete utilities) are the four the code reads, and the boundary it draws
    // — visible on the command line — is the one the code draws, since a script
    // file is passed through untouched. Too-tight, and the escapes are named
    // rather than implied: `eval`, a wrapper, an encoded here-doc.
    // Reopened 2026-08-01 by widening it to Windows. `cmd /c "echo x > f"` hides
    // its redirect behind quotes the way `python -c` does, and this crate's own
    // platform is the one where that is the ordinary spelling — so the guard was
    // reading the foreign half of its population and missing the native half.
    // Read against the function: the four shapes and the visible-on-the-command-
    // line boundary are unchanged; only the alphabet grew.
    // Re-read on 2026-08-04 after `writes_a_file` learned about wrappers. The
    // declaration is unchanged and still true: its boundary is what is visible
    // on the command line, and `rm` under `sudo` was always visible — the code
    // simply was not reading that far. Affirmed again.
    // Re-read on 2026-08-04 a second time: the affirmation above was recorded
    // before clippy asked for `contains` in place of `iter().any()` in the same
    // function, which moved the fingerprint without moving the rule. Read again,
    // still true, affirmed again.
    // Read on 2026-08-04 and affirmed. The listed subcommands are the ones
    // that reduce what this harness enforces, and each is checked below against
    // the CLI's own command list — a family whose members were made up would be
    // a guard about nothing.
    // Reopened 2026-08-05. The crossing beside it asked "is every listed
    // subcommand one the CLI has" and never asked the other direction: is every
    // name the CLI answers to for a listed verb itself listed. It was not.
    // `setup` carries visible_aliases = ["install", "tui"], so
    // "estigia install --uninstall" took Estigia out and classified Untouched,
    // the same operation as "estigia setup --uninstall" under a name the
    // product prints in its own help.
    //
    // Read against the code and still true. The family is unchanged — Estigia's
    // own subcommands that reduce what it enforces — and what grew is the set of
    // spellings for members it already had. The two verbs measured in the same
    // run and deliberately left out are named rather than implied: "sync"
    // rewrites the skill from this binary's copy and refuses an unrecognised
    // value rather than loosening anything, and "release" ends the oath instead
    // of lowering it, which the tracker records. Affirmed.
    ("disarming-shell", 14503526082505867704),
    // Read on 2026-08-04 and affirmed. The fragments name the files this
    // harness reads its own answers from, and every one is crossed against
    // what `resolve_paths` produces per adapter — five of eleven were wrong
    // when they were guessed, which is what that crossing is for.
    // Re-affirmed 2026-08-15: `gh`'s hosts file joined the population. It is not
    // a file Estigia's decisions are read from, which is the rule as stated, but
    // it decides which account every tracker call acts as — and issue 2's own
    // question was whether any write outside the repository can still reach
    // tracker state. This one can. The boundary is unchanged.
    // Re-affirmed again 2026-08-15, after review: the Windows spelling carried a
    // space, and `surface_of` splits a command on whitespace before matching, so
    // that entry could never fire through the shell — `Write` on the path
    // answered `Boundary` while `rm` on the same path answered `Routine`. Both
    // spellings now name the file without one. Population and boundary unchanged.
    // Re-affirmed 2026-08-15 for issue 26, and the population is genuinely wider
    // this time. Two entries changed and the rule did not: `.claude/settings.json`
    // became `.claude/settings`, because `contains` never reached
    // `settings.local.json` — the file an operator is told to put local overrides
    // in, read with the same authority, through which the gate could be switched
    // off; and `.claude/agents/` joined, because an agent definition is an
    // instruction with a tool allowlist, which is the same authority in another
    // shape. Both are files this harness's decisions are read from, so the rule as
    // stated already covered them and the list did not.
    //
    // The eleven instruction files are *not* in this list. They are derived from
    // the adapter table in `is_control_surface`, the way the skill tree is derived
    // from `skill::DIRECTORY` and for the same reason — a hand-spelled copy agrees
    // with the installer only until somebody renames one. This declaration covers
    // the literals; the derived set is crossed against `resolve_paths` per adapter
    // by `every_control_file_an_adapter_has_is_one_the_gate_measures`, which now
    // walks `paths.instructions` and did not before.
    ("control-surface", 6747193313278382829),
    // Reopened 2026-08-05 by measuring the enumeration instead of reading it.
    // Thirty command lines that visibly put bytes on disk were classified and
    // twenty-six came back Untouched, among them "wget -O src/main.rs URL" —
    // which is this module's own opening sentence with a different verb. The
    // sharpest were siblings of names already in the list: "unzip" and "tar"
    // classified as writes while "zip", "gzip", "xz" and "7z" did not, which is
    // not a boundary but a list that stopped halfway.
    //
    // Read against the function and still true. The boundary has not moved: it
    // is what is *visible on the command line*, and the side-effect writers the
    // declaration excludes by name (make, cargo build, npm install) were
    // measured in the same run and are still excluded. Curl and openssl are
    // read the way dd already was, by the flag that names the output rather
    // than by the command name. Only the alphabet grew, which is what
    // **too-tight** says is open. Affirmed.
    //
    // Reopened on 2026-08-06 and affirmed again. The declaration already names
    // "an interpreter handed code" as in the legitimate population, and the
    // boundary — what is *visible on the command line* — did not move: what
    // moved is that PowerShell's abbreviated switches now reach it. `-enc`,
    // `-ec`, `-comm` and `-Comman` were measured running their arguments while
    // the exact-spelling table held only `-command` and `-encodedcommand`, so a
    // whole shell's canonical spelling sat outside a population that claims it.
    // `-File` is still unmatched, which is the same declaration's other half:
    // an interpreter given a script file is not read. Only the alphabet grew.
    ("writing-shell", 2270012876963451618),
    // Read on 2026-08-04 and affirmed: the wrappers listed are the ones that
    // put another command's name after their own, and the command they run is
    // still visible on the line — which is `writing-shell`'s own boundary, not
    // an escape from it. `sudo rm -rf src` was missed by the list next door for
    // no better reason than the first word.
    ("running-shell", 13650568338058175168),
    // Read on 2026-08-05 and affirmed, on the round that noticed it was the one
    // boundary population in the classifier with no declaration at all — five
    // siblings in the same file carry one, and this list was added later
    // without one. Affirmed as **too-tight** knowingly: the three verbs are the
    // POSIX ways to make a control surface inert without writing to it, and the
    // declaration now names the Windows gap rather than leaving it as "a fourth
    // spelling". `icacls` is not in the list because nobody has watched it stop
    // a hook, and this crate does not add a spelling on reasoning alone.
    ("neutralising-shell", 7951848916686772554),
    // `delivery-phase` — new on 2026-08-01. Read against the constant: the five
    // spellings are the ones that land work, and the two that are absent —
    // `git push`, `gh pr create` — are absent for a reason the declaration
    // states, since gating the step that *reaches* review on being in review is
    // a deadlock. Exact equality rather than substring, so an operator's own
    // boundary is never guessed at. Too-tight, and the escape it leaves is the
    // one `irreversible-shell` already owns.
    ("delivery-phase", 10387013839543433198),
];

#[test]
fn nothing_writes_a_file_another_program_reads_by_truncating_it() {
    // Every file Estigia writes outside its own state belongs to somebody else
    // and is read while Estigia is writing: `settings.json` by the agent, the
    // skill by whatever assembles the prompt, the push hook by git. `fs::write`
    // truncates first, so a reader arriving in the window sees half a file — and
    // the failure lands on *their* program, at a moment that has nothing to do
    // with Estigia, looking like their bug.
    //
    // One writer, in `paths`, and this keeps a second one from appearing. The
    // rule is stated where it can be checked rather than in a comment somebody
    // has to find.
    // The three ways to put a file back at zero length. It read for `fs::write(`
    // alone, and `replace_atomically` does not use it — it opens with
    // `File::create` and renames — so the needle matched **nothing in shipping
    // code at all**, including the one file the exemption below was written for.
    // A guard whose pattern the codebase never uses cannot have caught anything,
    // and the spelling it was blind to is the one this crate actually writes.
    const TRUNCATES: &[&str] = &["fs::write(", "File::create(", ".truncate(true)"];
    let allowed = ["src/paths.rs"];
    let mut offenders = Vec::new();
    let mut examined = 0usize;
    let mut inside_the_writer = 0usize;
    for relative in sources() {
        // A test that plants a file is not writing somebody else's live
        // configuration, and a file that *is* a test module carries no
        // `#[cfg(test)]` line to split on.
        if relative.ends_with("tests.rs") {
            continue;
        }
        let source = std::fs::read_to_string(root().join(&relative)).expect("a readable source");
        // Tests plant files to be found; only the shipping code is held to this.
        let shipping = source.split("#[cfg(test)]").next().unwrap_or_default();
        let exempt = allowed.contains(&relative.as_str());
        if !exempt {
            examined += 1;
        }
        for (number, line) in shipping.lines().enumerate() {
            if !TRUNCATES.iter().any(|needle| line.contains(*needle)) {
                continue;
            }
            if exempt {
                inside_the_writer += 1;
            } else {
                offenders.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    // The exemption is not vacuous. This is the assertion whose absence let the
    // wrong needle sit here unnoticed: `src/paths.rs` is exempt *because* it is
    // the one place allowed to truncate, so if nothing in it matches, the
    // pattern is not reading what this crate writes with.
    assert!(
        inside_the_writer > 0,
        "nothing in `src/paths.rs` matches any of {TRUNCATES:?}, so the exemption covers nothing \
         and this guard is looking for a spelling the crate does not use"
    );
    assert!(
        examined > 30,
        "only {examined} shipping source(s) were read, which is fewer than this crate has \u{2014} \
         an empty walk finds no offenders and reads exactly like a clean one"
    );
    assert!(
        offenders.is_empty(),
        "these write in place instead of through `paths::replace_atomically`:
{}",
        offenders.join(
            "
"
        )
    );
}

/// A fixture may not answer a value that means *did not run*.
///
/// `tracker_rig` used to return `Option<TrackerRig>`, and all sixteen of its
/// callers opened with `let Some(rig) = tracker_rig() else { return; }`. In a
/// tree where the example it needs had not been built — which a **filtered**
/// `cargo test --test pipe` never builds — every one of them reported **pass**
/// having executed nothing. Measured at the commit before the fix, with the
/// fixture moved aside: `106 passed`, and one rig test alone in `0.00s` against
/// `0.42s` with it present.
///
/// Removing the `Option` fixed the sixteen the compiler could see. Nothing
/// stopped it coming back, and a reviewer said so: the fix was held by no test,
/// so the base commit *is* the fix turned off and the suite is green there.
///
/// Everything below is found from **the** definition of the rig, not from text
/// that reads like it: exactly one line in the file may pass the filter, and the
/// body runs from that line by index. Three decoy routes existed because those
/// two things were substring matches, and one of them took the signature check
/// itself from inside a `/* */` block. The filter is still a filter, so a real
/// definition written `pub fn` is invisible to it — `docs/honesty.md` measures
/// what that costs.
///
/// It holds four separate things, because reviewers walked past each in turn:
///
/// 1. **The whole signature.** A prefix match let `-> TrackerRigMaybe`, aliased
///    to `Option<TrackerRig>`, through.
/// 2. **Where the fixture is looked for.** The path was once
///    `CARGO_MANIFEST_DIR/target/debug/examples`, which does not exist under
///    `cargo test --release --target <triple>` — all six release lanes would have
///    hard-failed. Reverting that left the whole suite green, this guard
///    included, so it was a fix nothing held.
/// 3. **Ending the process instead of failing.** `std::process::exit(0)` made
///    cargo print no result line at all and exit 0 — worse than the defect this
///    guard is about, which at least claimed 106 passed. Scoped to the rig at
///    first, so a caller or a helper below it walked round; the whole file is the
///    scope now.
/// 4. **One spelling of a caller-side skip**: the `return` keyword written in a
///    test that reaches the rig. Not the skip itself — an earlier version of this
///    comment claimed that, and reviewers reproduced the whole defect through a
///    macro, a labelled `break`, and a second test file, all with this green.
///    `docs/honesty.md` lists every route with its measurement and whether it is
///    held.
///
/// It reads source text rather than types because each of these compiles
/// perfectly when it is wrong. **What holds the accidental path is the compiler**,
/// not this: `let Some(rig) = tracker_rig()` does not compile against a
/// non-`Option`, so nobody reintroduces the defect by copying a neighbour, which
/// is how sixteen callers came to have it. This catches a signature reverting, a
/// fixture looked for in the wrong place, a process ended instead of failed, and
/// the one skip spelling somebody is likely to reach for. Deliberate
/// circumvention is not in reach of reading text,
/// and pretending otherwise would be the same unmeasured claim this whole issue
/// has been about.
///
/// Not a lint against `Option` in fixtures generally — `repository()` in
/// `src/harness/guard/tests.rs` still answers one, and `docs/honesty.md` records
/// what that costs. This holds the one measured lying.
#[test]
fn the_tracker_rig_cannot_answer_that_it_did_not_run() {
    let source = std::fs::read_to_string(root().join("tests").join("pipe.rs"))
        .expect("the pipe suite is readable");

    // The whole signature, not a prefix of it. A reviewer walked past
    // `contains("-> TrackerRig")` with `type TrackerRigMaybe = Option<TrackerRig>`
    // and `fn tracker_rig() -> TrackerRigMaybe {`, which contains the needle and
    // answers an option — then removed the fixture and watched 106 tests pass on
    // nothing with this guard green.
    // Exactly one, because "a line that looks like the definition" is not the
    // definition. A reviewer put the signature inside a `/* */` block at column
    // zero — a comment cannot start with `//` there, so the filter took the decoy,
    // the real rig went back to `Option<TrackerRig>`, and 106 tests passed on
    // nothing with this green. A second one is a decoy by construction: the
    // callers can only bind to one.
    let defining: Vec<&str> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("fn tracker_rig("))
        .collect();
    assert_eq!(
        defining.len(),
        1,
        "tests/pipe.rs holds {} lines that read as a definition of `tracker_rig`, \
         so every check below would bind to whichever comes first: {defining:?}",
        defining.len()
    );
    let signature = defining[0];
    assert_eq!(
        signature.trim(),
        "fn tracker_rig() -> TrackerRig {",
        "`tracker_rig` no longer answers the rig itself, so a caller can be handed \
         a value meaning *did not run*"
    );

    // Where the fixture is looked for, and this is a guard the fix needed and did
    // not have. The path was once `CARGO_MANIFEST_DIR/target/debug/examples`,
    // which is absent under `cargo test --release --target <triple>` — so all six
    // release lanes would have hard-failed and no tag could have been cut. A
    // reviewer measured that reverting the derivation leaves the whole suite
    // green, this guard included, which by this repository's own rule made it an
    // untested fix.
    // From the definition line the check above found, not from the first text in
    // the file that reads like it. `split_once("fn tracker_rig()")` bound to
    // whichever came first — a reviewer put two comment lines mentioning the
    // rig near the top and every assertion below read those four lines instead,
    // passing while the real rig did both of the things they refuse.
    // From the line, not from text that reads like it. `split_once(signature)`
    // matched the first *substring*, so a `//` comment quoting the signature took
    // the split even though the check above rejects it as a definition. Lines
    // carry no such ambiguity: exactly one defines the rig, and the body runs
    // from it to the next line that is only a closing brace.
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|line| *line == signature)
        .expect("the definition line came out of this file");
    let past = lines
        .iter()
        .skip(at)
        .position(|line| line.trim_end() == "}")
        .expect("the rig has an end");
    let body = lines[at..=at + past].join("\n");
    let body = body.as_str();
    assert!(
        body.contains("current_exe()"),
        "the fixture is no longer located from the running test binary, so it is \
         being looked for under one profile while cargo built it under another"
    );
    assert!(
        !body.contains("CARGO_MANIFEST_DIR"),
        "the fixture is located from the manifest again, which names `target/debug` \
         however the suite was actually built"
    );
    // Nothing in this suite may end the process instead of failing. A reviewer
    // replaced the rig's assertion with `eprintln!` plus `std::process::exit(0)`:
    // the fixture absent, `cargo test --test pipe` printed `running 106 tests`, no
    // result line at all, and exited 0 — worse than the defect this issue is named
    // for, which at least claimed 106 passed. Scoped to the rig's body at first,
    // which two reviewers then walked round by putting the call in a caller or in
    // a helper below the rig. The whole file is the scope the sentence always
    // meant — and it catches `use std::process::exit as leave;` too, because that
    // line contains the substring. `use std::process::{exit as leave};` does not,
    // and that is in `docs/honesty.md` rather than chased with a second substring.
    let exiting = source
        .lines()
        .find(|line| line.contains("process::exit"))
        .map(str::trim);
    assert!(
        exiting.is_none(),
        "this suite ends the process instead of failing, which cargo reports as \
         success with no test result at all: {exiting:?}"
    );

    // Per test function, not per line. Both routes a reviewer found are
    // caller-side: splitting `let Some(rig) = tracker_rig() else` across two
    // lines defeats a line-wise scan, and a bare `if !… { return; }` before the
    // call needs no option at all. A test that reaches the rig has no business
    // returning early for any reason, so that is what is asserted.
    let mut skipping = Vec::new();
    for chunk in source.split("\n#[test]") {
        if !chunk.contains("tracker_rig()") {
            continue;
        }
        let name = chunk
            .lines()
            .find(|line| line.trim_start().starts_with("fn "))
            .unwrap_or("<unnamed>")
            .trim();
        // Comments are not code, and this file quotes the very pattern being
        // refused — the rig's own doc comment carries `else { return; }` to say
        // what it used to be. Scanning the raw text made that a finding against
        // the test the rig happens to sit behind.
        let code = chunk
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        // The keyword, not two spellings of it: `return;` and `return }` were
        // matched literally at first and `return Default::default();` walked past
        // them by two words, with every gate in this repository green.
        //
        // The whole line, string literals included. A version of this cut strings
        // out first, to spare an assertion message with the word in it — and a
        // reviewer measured what that cost: the stripper toggled on any `"`,
        // including a char literal, so `let _sep = '"'; if !built { return; }` hid
        // a plain `return;` and put 106 tests back to passing on nothing. It was
        // guarding against a hazard that cannot happen here either, since this
        // reads `tests/pipe.rs` and the messages that say "did not return" are in
        // `src/tui/models.rs`, which it never opens. A false positive over prose
        // is cheap and visible; a false negative over the one spelling this exists
        // to catch is neither.
        //
        // So both false positives stand and are declared: a `return` in a string
        // literal, and one inside a **closure**, which cannot skip the test at
        // all. Telling either from a real skip needs to know what a closure and a
        // literal are, which is where reading text ends.
        let offender = code.lines().find(|line| {
            line.split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == "return")
        });
        if let Some(line) = offender {
            skipping.push(format!("{name}  ->  {}", line.trim()));
        }
    }
    assert!(
        skipping.is_empty(),
        "these reach the rig and write `return`, which is how sixteen tests came to \
         report pass without running. If the word is inside a closure or a string \
         rather than in the test's own control flow, it cannot skip anything and \
         this is a known false positive: put that line in a helper outside the test, \
         or rephrase the message. `docs/honesty.md` records both cases and why \
         neither is worth a semantic parser: {skipping:#?}"
    );

    // The exemption is not vacuous: the callers have to be there for their
    // absence to mean anything. Sixteen at the time this was written — and
    // counted as call sites in code, which is not what `matches()` over the raw
    // file returns. That counted seventeen, because the definition line contains
    // the needle, and it counted commented-out callers too: a reviewer commented
    // out all sixteen and this still passed.
    let callers = source
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with("//") && !line.starts_with("fn ") && line.contains("tracker_rig()")
        })
        .count();
    assert!(
        callers >= 12,
        "only {callers} call site(s) of `tracker_rig` found in code, so this guard \
         is reading a spelling the suite no longer uses and would pass on a file \
         where nothing calls the rig at all"
    );
}
