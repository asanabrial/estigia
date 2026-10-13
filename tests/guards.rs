// An assertion that panics is the assertion working. See `tests/pipe.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Population declarations, read as data rather than as prose.
//!
//! A guard is a rule about a **population**: the set of things it is supposed to
//! catch. Whether it catches them is a claim, and a claim written in a comment
//! decays the moment the code beneath it moves — silently, because a comment
//! never fails to compile.
//!
//! So each declaration is a `pub const … : Population` sitting in `src/` —
//! see [`estigia::harness::population`] — parsed out with `syn`, resolved to
//! the real item it names in its `subject` field, and **fingerprinted together
//! with that item's own tokens**. Changing either one reopens the claim: the
//! fingerprint stops matching, this test fails, and the author has to look at
//! the declaration again before recording the new one.
//!
//! `subject` replaces what used to be implicit in where a doc comment sat: a
//! declaration used to bind to whatever item its comment happened to be
//! attached to, so moving the comment moved the binding with it and nothing
//! ever checked that the two still belonged together. Naming the subject makes
//! the binding a fact this test resolves and can fail on — a `subject` that
//! names no real const, static, fn, enum or struct in the same file is refused
//! loudly, naming the file and the subject.
//!
//! `disposition` is `Disposition`, not a string: `TooTight`, `TooLoose` and
//! `FailClosed` are the only three ways a population can be wrong, and an
//! invented fourth is a compile error in `src/` rather than a value this test
//! has to reject at run time — the closed vocabulary the old string list held
//! is now the type system's job, not this file's.
//!
//! # The proof boundary
//!
//! This proves that every declaration is well formed, that its `subject`
//! resolves to a real item, and that neither the rule nor the code under it has
//! changed since somebody last affirmed the pair. It does **not** prove any
//! declaration is true, and it does not prove every guard that needs one has
//! one — a guard with no declaration is invisible to this test, exactly as it
//! is to a reader.
//!
//! And it fingerprints **the subject's own tokens**, which for most of them is
//! the list. A declaration on a list therefore holds the rule and the
//! population, and not the code that matches against them: `control-surface`
//! says *matched on path fragments* and names `CONTROL_SURFACE` as its
//! subject, so changing `is_control_surface` leaves this test green on its own.
//! What holds that matcher is the **second** declaration beside the list, whose
//! subject is `CONTROL_SURFACE_CORPUS`: a checked-in corpus of paths and the
//! answer both roads must give each one, asserted by
//! `the_control_surface_matcher_answers_its_corpus` in `src/harness/tests.rs`. A
//! matcher edit that moves an answer reddens that test; recording the new answer
//! moves this declaration's fingerprint and reopens the claim. The rule half
//! still bites — the text is hashed together with the subject's tokens — so a
//! boundary sentence cannot drift silently, only the code beneath one can, and
//! for `control-surface` that code is now named by a corpus.

use std::collections::BTreeMap;
use std::path::PathBuf;

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

/// One declaration, and the item it is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    family: String,
    disposition: String,
    /// The population and its boundary, as written.
    text: String,
    /// The subject, named for the failure message: `relative :: kind name`.
    item: String,
    /// A hash over the declaration **and** the subject's tokens.
    fingerprint: u64,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The whole `pipe` suite as one body of text: the root file plus every module
/// beside it. The suite is split across `tests/pipe/` and the root keeps the
/// shared fixtures, so a reader that opened only one of the two would miss the
/// rig or its callers.
fn pipe_sources() -> String {
    let tests = root().join("tests");
    let mut paths = vec![tests.join("pipe.rs")];
    let directory = tests.join("pipe");
    if directory.is_dir() {
        let mut modules: Vec<PathBuf> = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()))
            .map(|entry| entry.expect("a readable entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect();
        modules.sort();
        paths.extend(modules);
    }
    paths
        .iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()))
        })
        .collect::<Vec<_>>()
        .join("\n")
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

/// Whether `ty` names `Population`, however it was reached (`Population` or a
/// path ending in it).
fn is_population_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Population")
}

/// A short kind label and bare name for an item this walker can resolve a
/// `subject` against, or bind a body to.
fn subject_of(item: &syn::Item) -> Option<(&'static str, String)> {
    Some(match item {
        syn::Item::Const(node) => ("const", node.ident.to_string()),
        syn::Item::Static(node) => ("static", node.ident.to_string()),
        syn::Item::Fn(node) => ("fn", node.sig.ident.to_string()),
        syn::Item::Enum(node) => ("enum", node.ident.to_string()),
        syn::Item::Struct(node) => ("struct", node.ident.to_string()),
        _ => return None,
    })
}

/// Everywhere a declaration or a subject can be found, other than a
/// top-level item: an associated const inside an `impl` block. `syn::Item`
/// has no variant for one — it lives in `syn::ImplItem` — so this walker
/// carries the two shapes side by side rather than only ever looking at
/// `parsed.items`, which is top level and nothing under it.
enum Found<'a> {
    TopLevel(&'a syn::Item),
    ImplConst(&'a syn::ImplItemConst),
}

impl<'a> Found<'a> {
    fn subject(&self) -> Option<(&'static str, String)> {
        match self {
            Found::TopLevel(item) => subject_of(item),
            Found::ImplConst(node) => Some(("const", node.ident.to_string())),
        }
    }

    /// The type ascription, for the ones that are typed at all: only a const
    /// (top level or associated) ever declares a `Population`.
    fn const_type(&self) -> Option<&syn::Type> {
        match self {
            Found::TopLevel(syn::Item::Const(node)) => Some(&node.ty),
            Found::ImplConst(node) => Some(&node.ty),
            _ => None,
        }
    }

    /// The initialiser expression of a const, top level or associated.
    fn const_expr(&self) -> Option<&syn::Expr> {
        match self {
            Found::TopLevel(syn::Item::Const(node)) => Some(&node.expr),
            Found::ImplConst(node) => Some(&node.expr),
            _ => None,
        }
    }

    fn const_vis(&self) -> Option<&syn::Visibility> {
        match self {
            Found::TopLevel(syn::Item::Const(node)) => Some(&node.vis),
            Found::ImplConst(node) => Some(&node.vis),
            _ => None,
        }
    }

    /// The subject's own tokens: what the fingerprint hashes together with
    /// the declaration. See the comment on the call site in `declarations`.
    fn body(&self, relative: &str, const_name: &str) -> String {
        match self {
            Found::TopLevel(item) => match item {
                syn::Item::Const(node) => tokens(&node.expr),
                syn::Item::Enum(node) => tokens(&node.variants),
                syn::Item::Fn(node) => tokens(&node.block),
                syn::Item::Struct(node) => tokens(&node.fields),
                syn::Item::Static(node) => tokens(&node.expr),
                other => panic!(
                    "{relative} :: const {const_name} resolves `subject` to an item kind this \
                     walker cannot read the body of ({other:?}); teach `Found::body` how to \
                     read it, or point the declaration at an item it can."
                ),
            },
            Found::ImplConst(node) => tokens(&node.expr),
        }
    }
}

/// Every top-level item, every associated const inside an `impl` block, and
/// every item inside an inline `mod { .. }` body — recursively, since a `mod`
/// can hold another `mod`. A declaration or a subject written in any of
/// these was previously invisible to this walker, which only ever read
/// `parsed.items`: a `Population` inside an `impl` or a nested `mod` was
/// parsed by nobody, fingerprinted by nobody and affirmed by nobody, while
/// reading on the page exactly like the ones this test did cover.
///
/// What this does **not** reach is a declaration inside a function body — a
/// local `const` there is a `syn::Stmt`, not a `syn::Item`, and walking every
/// statement of every function to find one is not worth the cost for a
/// declaration that would have to be written somewhere nobody would ever
/// look for a population. `population_text_occurrences` below is the honest
/// answer to that gap: it cross-checks this walker's count against the raw
/// text and fails loudly, naming the file, the moment the two disagree.
fn collect_found<'a>(items: &'a [syn::Item], out: &mut Vec<Found<'a>>) {
    for item in items {
        match item {
            syn::Item::Mod(node) => {
                if let Some((_, inner)) = &node.content {
                    collect_found(inner, out);
                }
                out.push(Found::TopLevel(item));
            }
            syn::Item::Impl(node) => {
                for impl_item in &node.items {
                    if let syn::ImplItem::Const(const_item) = impl_item {
                        out.push(Found::ImplConst(const_item));
                    }
                }
                out.push(Found::TopLevel(item));
            }
            other => out.push(Found::TopLevel(other)),
        }
    }
}

/// A `Population` struct literal's fields, read out of the initialiser.
struct Fields {
    family: String,
    disposition: String,
    subject: String,
    text: String,
}

fn expect_str(expr: &syn::Expr, relative: &str, const_name: &str, field: &str) -> String {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(literal),
        ..
    }) = expr
    {
        literal.value()
    } else {
        panic!(
            "{relative} :: const {const_name}.{field} is not written as a plain string literal, \
             so this reader cannot take its value"
        );
    }
}

/// Reads the **variant name** out of source text — a `syn::Expr::Path` has no
/// runtime value to call `as_str` on — then hands it to the real enum, so the
/// kebab-case spelling itself has exactly one source: `Disposition::as_str`.
/// A copy of the mapping written here a second time is the thing this reader
/// exists to not be.
fn expect_disposition(expr: &syn::Expr, relative: &str, const_name: &str) -> String {
    let syn::Expr::Path(path) = expr else {
        panic!(
            "{relative} :: const {const_name}.disposition is not a `Disposition::` path, so \
             this reader cannot take its value"
        );
    };
    let Some(variant) = path.path.segments.last() else {
        panic!("{relative} :: const {const_name}.disposition names no variant");
    };
    use estigia::harness::population::Disposition;
    let disposition = match variant.ident.to_string().as_str() {
        "TooTight" => Disposition::TooTight,
        "TooLoose" => Disposition::TooLoose,
        "FailClosed" => Disposition::FailClosed,
        other => panic!(
            "{relative} :: const {const_name}.disposition names `{other}`, which is not a \
             `Disposition` variant this reader knows. Either `src/harness/population.rs` grew a \
             new one and this file was not taught it, or the source does not compile."
        ),
    };
    disposition.as_str().to_owned()
}

fn parse_population(expr: &syn::Expr, relative: &str, const_name: &str) -> Fields {
    let syn::Expr::Struct(literal) = expr else {
        panic!(
            "{relative} :: const {const_name} is typed `Population` but is not initialised with \
             a `Population {{ .. }}` struct literal"
        );
    };
    let mut family = None;
    let mut disposition = None;
    let mut subject = None;
    let mut text = None;
    for field in &literal.fields {
        let syn::Member::Named(name) = &field.member else {
            continue;
        };
        match name.to_string().as_str() {
            "family" => family = Some(expect_str(&field.expr, relative, const_name, "family")),
            "subject" => subject = Some(expect_str(&field.expr, relative, const_name, "subject")),
            "text" => text = Some(expect_str(&field.expr, relative, const_name, "text")),
            "disposition" => {
                disposition = Some(expect_disposition(&field.expr, relative, const_name));
            }
            _ => {}
        }
    }
    Fields {
        family: family
            .unwrap_or_else(|| panic!("{relative} :: const {const_name} has no `family` field")),
        disposition: disposition.unwrap_or_else(|| {
            panic!("{relative} :: const {const_name} has no `disposition` field")
        }),
        subject: subject
            .unwrap_or_else(|| panic!("{relative} :: const {const_name} has no `subject` field")),
        text: text
            .unwrap_or_else(|| panic!("{relative} :: const {const_name} has no `text` field")),
    }
}

/// Every declaration in the crate, bound to the item its `subject` names.
///
/// Reads every top-level item, every associated const inside an `impl`, and
/// every item inside an inline `mod { .. }` — see `collect_found` — and
/// resolves `subject` against that whole set, not only the top level. A
/// declaration and its subject can sit in different scopes of the same file
/// (a `Population` at the top naming a method inside an `impl`, say), so the
/// resolution pool is the file's full flattened list rather than one scope.
fn declarations() -> Vec<Declaration> {
    let mut found = Vec::new();
    for relative in sources() {
        let path = root().join(&relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        let parsed = syn::parse_file(&source)
            .unwrap_or_else(|error| panic!("{} must parse: {error}", path.display()));

        let mut candidates = Vec::new();
        collect_found(&parsed.items, &mut candidates);

        for candidate in &candidates {
            let Some(ty) = candidate.const_type() else {
                continue;
            };
            if !is_population_type(ty) {
                continue;
            }
            let const_name = candidate
                .subject()
                .map(|(_, name)| name)
                .unwrap_or_default();
            let expr = candidate
                .const_expr()
                .expect("a `const_type` match always has a `const_expr` too");
            let fields = parse_population(expr, &relative, &const_name);

            let matching: Vec<(&str, &Found)> = candidates
                .iter()
                .filter_map(|other| {
                    let (kind, name) = other.subject()?;
                    (name == fields.subject).then_some((kind, other))
                })
                .collect();
            assert!(
                matching.len() <= 1,
                "{relative} :: const {const_name} declares `subject: \"{}\"`, and {} items in \
                 {relative} answer to that name — a fingerprint bound to whichever one is found \
                 first would affirm a claim against code nobody read; rename one of them so the \
                 subject is unique in the file.",
                fields.subject,
                matching.len()
            );
            let Some((kind, subject_found)) = matching.into_iter().next() else {
                panic!(
                    "{relative} :: const {const_name} declares `subject: \"{}\"`, and no const, \
                     static, fn, enum or struct named `{}` exists in {relative} — a `Population` \
                     that names no real item binds to nothing, and this walker would otherwise \
                     fingerprint prose alone.",
                    fields.subject, fields.subject
                );
            };

            // The subject's own tokens, so the declaration text and the code
            // it describes contribute separately and a reader can tell which
            // of the two moved.
            //
            // Tokens rather than `Debug`: `syn`'s `Debug` prints spans, and a
            // fingerprint taken from it moves when a comment near the subject
            // is edited — a claim reopened for a reason this module's own
            // documentation says would not reopen it. A token stream carries
            // no positions, so reformatting and comments are invisible and a
            // changed expression is not.
            let body = subject_found.body(&relative, &const_name);

            let fingerprint_input = format!(
                "{relative}|{}|{}|{}|{}|{body}",
                fields.subject,
                fields.family,
                fields.disposition,
                fields.text.trim()
            );
            found.push(Declaration {
                fingerprint: fingerprint(&fingerprint_input),
                family: fields.family,
                disposition: fields.disposition,
                text: fields.text.trim().to_owned(),
                item: format!("{relative} :: {kind} {}", fields.subject),
            });
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
        // The disposition itself is not asserted against a list here: it came
        // from a `Disposition::` path, and `expect_disposition` already
        // refused anything but the three real variants, or the crate would
        // not have compiled. That is the trade `Population` was written for —
        // an invented disposition is a compile error in `src/`, not a value
        // this test discovers at run time.
        assert!(
            declaration.text.len() > 30,
            "{}: the declaration names no population and no boundary: {:?}",
            declaration.item,
            declaration.text
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
fn every_population_const_is_public() {
    // `pub`, not `pub(crate)`: a `pub` item in a lib target is never dead code,
    // so these consts do not need `#[allow(dead_code)]`, and `cargo doc
    // --no-deps` publishes what each guard covers. A `pub(crate)` declaration
    // would compile, pass every other check here, and simply never be read by
    // anyone outside the crate — which is the one property this test exists to
    // hold, and nothing above it checks visibility at all.
    for relative in sources() {
        let path = root().join(&relative);
        let source = std::fs::read_to_string(&path).expect("a readable source");
        let parsed = syn::parse_file(&source).expect("the file parses");
        let mut candidates = Vec::new();
        collect_found(&parsed.items, &mut candidates);
        for candidate in &candidates {
            let Some(ty) = candidate.const_type() else {
                continue;
            };
            if !is_population_type(ty) {
                continue;
            }
            let name = candidate
                .subject()
                .map(|(_, name)| name)
                .unwrap_or_default();
            assert!(
                matches!(candidate.const_vis(), Some(syn::Visibility::Public(_))),
                "{relative} :: const {name} is a `Population` that is not `pub`",
            );
        }
    }
}

/// How many times `text` writes a `Population` type ascription, counted on
/// the text itself rather than on what `syn` resolved — so a declaration
/// `collect_found` cannot reach (a local `const` inside a function body) is
/// still counted, and the count below can be compared against it.
///
/// A whole-text substring search rather than a per-line filter, because the
/// two halves of one declaration can be on different lines: `rustfmt` breaks
/// `pub const NAME: Population =` from the `Population { .. }` literal that
/// follows it when the name is long, and a filter that requires both halves
/// on one line misses exactly that spelling.
fn population_text_occurrences(text: &str) -> usize {
    text.matches(estigia::harness::population::DECLARATION_ASCRIPTION)
        .count()
}

#[test]
fn no_population_declaration_hides_from_the_walk() {
    // The honest replacement for a guard this issue deleted: rather than
    // trusting that widening `collect_found` into `impl` blocks and `mod`
    // bodies reaches everywhere a declaration could be written, this counts
    // the text's own occurrences of the type ascription and fails, naming
    // the file, the moment that count is higher than what the `syn` walk
    // found — which is what a declaration inside a function body would do,
    // since `collect_found` does not walk statements.
    for relative in sources() {
        let path = root().join(&relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
        let stripped = estigia::source_text::code_of(&source);
        let text_count = population_text_occurrences(&stripped);

        let parsed = syn::parse_file(&source)
            .unwrap_or_else(|error| panic!("{} must parse: {error}", path.display()));
        let mut candidates = Vec::new();
        collect_found(&parsed.items, &mut candidates);
        let walk_count = candidates
            .iter()
            .filter(|candidate| candidate.const_type().is_some_and(is_population_type))
            .count();

        assert!(
            text_count <= walk_count,
            "{relative}: the text writes `: Population =` {text_count} time(s) but the walk \
             found only {walk_count} — a `Population` declaration is written somewhere this \
             walker cannot see it (a `const` inside a function body, for instance); move it to \
             an item `collect_found` reaches: top level, an `impl` block, or a `mod` body"
        );
    }
}

#[test]
fn a_declaration_and_its_code_have_not_moved_since_they_were_affirmed() {
    // The mechanism the handoff asked for: the claim is frozen against the item
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
/// beside this code and decided it was still true. The dated note under an
/// entry is when that reading happened — it is the record the table exists to
/// hold, not commentary on the code beside it. Regenerating the table without
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
    //
    // Re-read on 2026-08-28: the declaration moved from a doc comment into
    // `WRITE_TOOLS_POPULATION`, a `pub const Population` sitting immediately
    // before `WRITE_TOOLS`. Same words, only the carrier changed, so the
    // fingerprint moved for that reason alone and the population itself was
    // re-read and is still true.
    ("write-tools", 13786488437526146739),
    // `shell-tools` — split out of `write-tools` while reading it. These are the
    // tools whose *argument* decides what they are: `git status` through one is
    // a read and `git push` through the same one is a boundary, so they cannot
    // share a list with tools that are writes by construction.
    //
    // Re-read on 2026-08-28 for the same reason as `write-tools`: moved into
    // `SHELL_TOOLS_POPULATION`, words unchanged, still true.
    ("shell-tools", 7235849314805658990),
    // `irreversible-shell` — relabelled from fail-closed while reading it. A
    // spelling that is not on the list escapes, and escaping is not failing
    // closed. What is fail-closed is the substring *matching*, which is a
    // different claim and now says so.
    // Re-read on 2026-08-14 and still true, and narrower than it was. The match
    // now ends at a word: `contains` alone refused `git merge-base
    // --is-ancestor` as `git merge`, which is a read this crate's own transport
    // performs. The claim is unchanged — these spellings are gated, and they are
    // not the only spellings — while one class of false refusal is gone.
    //
    // Re-read on 2026-08-28: moved into `IRREVERSIBLE_POPULATION`, words
    // unchanged, still true.
    //
    // Re-read on 2026-09-07: `git pull` joined the list. It was reaching
    // `Action::Untouched` — `git pull --no-rebase` runs `git merge`
    // internally and nothing gated it. It is not in `DELIVERS`: pulling is an
    // integration step, not a landing one, and the claim this population
    // makes — these spellings are gated, and they are not the only spellings
    // — is unchanged by adding one more gated spelling.
    //
    // Re-read on 2026-09-13: `git tag -l`, `git tag --list` and bare `git tag`
    // list and left the list's meaning — every other `git tag` spelling,
    // including `git tag -d`, stays gated. Still too-tight, still true.
    ("irreversible-shell", 3274127317113882223),
    // `repository-shell` — the git commands that write. Same shape and same
    // limits as `write-tools`, and it exists so that composing `git worktree
    // add` by hand is gated rather than invisible.
    // Reopened 2026-08-01 by narrowing it. It had claimed "every spelling of a
    // repository write reachable from a shell" — a population it never covered,
    // since a redirect and a `sed -i` write the same tree without naming git.
    // Now it claims only the spellings that name git, which is what the list
    // holds. Read against the constant: still too-tight, and the escapes named
    // (an alias, a wrapper script) are the ones that remain.
    //
    // Re-read on 2026-08-28: moved into `REPOSITORY_SHELL_POPULATION`, words
    // unchanged, still true.
    //
    // Re-read on 2026-10-05: `git switch --create` and `git switch --force-create`
    // joined it. They create a ref exactly as
    // `git switch -c` does, and the claim — these spellings are gated, and they
    // are not the only spellings — is unchanged by adding two more gated
    // spellings.
    //
    // Re-read on 2026-10-07: `git switch --orphan` joined it. It
    // creates a ref exactly as `git switch -c` does, and the claim is unchanged
    // by adding one more gated spelling.
    ("repository-shell", 11936916847651860326),
    ("issue-creation-shell", 3013093601863561019),
    // `exit-code` — genuinely fail-closed. `0` to `5`
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
    //
    // Re-read on 2026-08-28: moved into `TRANSLATE_POPULATION`, a `pub const`
    // immediately before `translate`, words unchanged, still true.
    //
    // Re-read on 2026-08-29: `translate`'s final `Refusal` literal picked up
    // `observed: None` when `Refusal` gained that field. The contract this
    // population claims — codes `0` through `5`, everything else fail-closed
    // to `Unknown` — is untouched; the added field carries what the *gate*
    // read, and `translate` answers for the transport, which never sets it.
    //
    // Re-read on 2026-09-11: the `2` arm carries the transport's own `action`
    // where the table has no row, and the fallback detail carries the
    // envelope's scalar fields. Codes `0` through `5` and the `_` arm to
    // `Unknown` are unchanged, so the population and its boundary still hold.
    //
    // Re-read on 2026-09-27: the committed arm's fallback detail now says *this
    // operation had already written* rather than *this call*, because a retry
    // resuming an earlier call's marker takes the same arm. Codes
    // `0` through `5` and the `_` arm to `Unknown` are unchanged, so the
    // population and its boundary still hold.
    //
    // Re-read on 2026-10-06: `translate` also sets the refusal's `class` from the
    // exit code and the class table. Codes `0` through `5`, and the
    // `_` arm to `Unknown`, are untouched: the new field reads the same exit code
    // and maps `5` and the unnamed codes to `unknown`, never to a named state.
    // Re-read the next day, after review: only exit `1` reads the class table, so a
    // code outside `0` to `5` is `unknown` in class as it already was in outcome.
    ("exit-code", 929166899009124012),
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
    //
    // Re-read on 2026-08-28: moved into `DISARMS_THE_HARNESS_POPULATION`, words
    // unchanged, still true. (This paragraph's own history belongs to
    // `disarming-shell`, carried here from the file layout that used to hold
    // both under one comment block; see that entry below.)
    //
    // Re-read on 2026-09-27. Two members joined rather than a
    // rule moving: `estigia config edit` and `estigia config forget` write what
    // the listed `estigia config set` writes — the installed contract, and a
    // repository's own rows — and neither matched any population, so the gate
    // answered `Untouched` and a run could reconfigure the harness it was
    // running under. The declaration's text gained one sentence naming the
    // crossing that now holds every leaf under a listed verb to this list or to
    // `DISARM_READS`, so the fingerprint moved for that reason too. The family's
    // claim is unchanged: these spellings are gated, and the boundary — a
    // substring match, so a spelling that reaches the same effect another way
    // is not — is unchanged. Affirmed.
    ("disarming-shell", 16261140763946589303),
    // Added on 2026-09-27. `estigia config list` and `estigia
    // config repos` are the subcommands of a verb the disarm list names that
    // only read; the crossing that holds every leaf under such a verb to the
    // list needs the other side declared somewhere a test can read, and this is
    // that declaration. Read against the two handlers: neither writes a
    // contract, a repository file or the registry — `config list` prints rows
    // and `config repos` filters the recorded list as it reads it, and the
    // pruning never rewrites the file. Deliberately `FailClosed` rather than
    // too-tight: a leaf outside both lists fails the crossing rather than being
    // cleared, so a third writing subcommand is a decision rather than an
    // omission.
    ("disarm-reads", 2044718300305756683),
    // Re-read on 2026-10-07. The verb and the `estigia` word
    // are now matched on a dequoted copy as the record matchers already were,
    // so `estigia stand\-down`, `st''and-down` and `es''tigia stand-down` no
    // longer split past the matcher while the shell runs them. The declaration
    // says so, and the population is unchanged: an alias, a wrapper script or a
    // renamed binary that never writes `estigia <verb>` as two words is still
    // not matched. Affirmed.
    ("person-only-shell", 15023960501866845408),
    // Read on 2026-08-04 and affirmed. The fragments name the files this
    // harness reads its own answers from, and every one is crossed against
    // what `resolve_paths` produces per adapter — five of eleven were wrong
    // when they were guessed, which is what that crossing is for.
    // Re-affirmed 2026-08-15: `gh`'s hosts file joined the population. It is not
    // a file Estigia's decisions are read from, which is the rule as stated, but
    // it decides which account every tracker call acts as — and the stand-aside's own
    // question was whether any write outside the repository can still reach
    // tracker state. This one can. The boundary is unchanged.
    // Re-affirmed again 2026-08-15, after review: the Windows spelling carried a
    // space, and `surface_of` splits a command on whitespace before matching, so
    // that entry could never fire through the shell — `Write` on the path
    // answered `Boundary` while `rm` on the same path answered `Routine`. Both
    // spellings now name the file without one. Population and boundary unchanged.
    // Re-affirmed 2026-08-15, and the population is genuinely wider
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
    // Re-affirmed 2026-08-15 after four blind reviews of the same head, and this
    // time the **rule itself was restated** because the population genuinely
    // outgrew it. A reviewer put it precisely: the gate reads nothing from
    // `~/.claude/CLAUDE.md`, so a write there changes what the agent is *told*
    // and not what the binary *does* — the declared rule covered enforcement
    // inputs and the instruction files are compliance inputs. They belong, for
    // the reason `README.md` gives about this whole crate, and the declaration
    // now names both kinds instead of stretching one over the other.
    //
    // Re-affirmed again after four more blind reviews of the same head. The
    // population is wider and the *shape* of the entries changed, so the claim is
    // read rather than carried:
    //
    // - `gemini/settings.json` beside the POSIX spelling, because that is where
    //   the gate is registered on Windows. Found by the crossing, once it walked
    //   every platform instead of the host's.
    // - `opencode/agents`, `opencode/plugins`, `opencode/opencode.json` and
    //   `crush/crush.json` — anchored to what the installer writes. The
    //   `.config/` prefix they carried is moved by `XDG_CONFIG_HOME`; dropping it
    //   entirely, which was the first attempt, left `opencode` as a bare
    //   directory name gating `node_modules/opencode/**` and any checkout named
    //   that. Both ends were wrong. Found by reviewers, not by the crossing —
    //   which walks two XDG layouts now, so it would be.
    // - The trailing slashes are all still here, and now mean something: a
    //   fragment ending in `/` names a directory, and `names` matches it by what
    //   is under it or by itself. They spent one head removed — `surface_of`
    //   appends a separator, so with a slash the bare directory was `Routine` on
    //   the write road while `rm <dir>` was `Boundary` — and removing them
    //   traded that for `.estigiaignore` and `skills/flow.md` becoming
    //   `Boundary`. Neither trade was necessary once the matcher read the slash.
    // - The four rules **directories**. Gating Estigia's filename inside a
    //   directory the host reads whole is defeated by a neighbour, and the
    //   restated population clause is exactly what a neighbour changes.
    // Re-affirmed once more, and the reason is the entry's own shape rather than
    // its members: a fragment ending in `/` now names a **directory**, matched by
    // what is under it or by itself — and not by a name that merely ends alike,
    // nor by one that starts alike, since a directory fragment is anchored on
    // the left exactly as a dot-fragment is. Anchoring only the dot ones left
    // the two roads disagreeing on the entries this change added. The file
    // fragments without a dot stay unanchored because `cli/hosts.yml` has to
    // match mid-segment; `docs/honesty.md` records what that leaves.
    // Both earlier spellings were wrong in opposite directions — with the slash
    // the bare directory was `Routine` on the write road while `rm <dir>` was
    // `Boundary`; without it, `.estigiaignore` and `skills/flow.md` became
    // `Boundary`. The boundary sentence above says what a false positive costs and
    // now says what that is in seconds, because the population reaches a project's
    // own files and the old one-line account of the trade was written when it did
    // not.
    //
    // `.config/opencode/` is back beside the tails that cover the relocated root.
    // Replacing it with them alone was a **loosening**: what Estigia does not
    // write under that root lost its boundary, which is an existing entry's
    // sensitivity changing, and it was recorded as an over-gating fix until a
    // reviewer measured it. No count here on purpose — the set is whatever
    // OpenCode itself keeps there, three drafts said *nine* without anybody
    // enumerating them, and `docs/honesty.md` holds the shapes that were
    // actually measured. A number in two files is a number that disagrees.
    // Reopened once more, and by a member this time rather than the rule:
    // `.opencode/agents/`. `roles::definition_for` searches the repository's own
    // OpenCode definition root beside its Claude one, in a single `vec!`, and the
    // two were gated by different fragments — `.claude/agents/` reached the
    // first, and `opencode/agents/` is anchored on the left, so nothing reached
    // the second. A reviewer measured it `Routine` on both roads while
    // `docs/honesty.md` and the pull request both said every definition root was
    // watched. A definition not found is `Ok(None)`, which `declared_policy`
    // reads as *every tool allowed*, so the file that writes an agent its own
    // allowlist was the one riding the renewal window.
    // Reopened 2026-08-15, and this time by the **shape** of the list
    // rather than by any one member. Read against the declaration and affirmed:
    // the rule is unchanged — *any path a write to which changes what this
    // harness enforces, or what an agent is told this harness may enforce* — and
    // the population is wider in three ways, each of which the rule as stated
    // already covered while the list did not.
    //
    // - Three members **left** this list and are not a loosening. `.cline/rules/`,
    //   `.continue/rules/` and `windsurf/memories/` were derived, by truncating
    //   the instruction fragment each of those adapters declared. Continue and
    //   Cline were retired as installable hosts, so their two directories
    //   became literals again, and the same happened to Windsurf's and
    //   removed the derivation: no answer moved, and a future adapter's rules
    //   directory is no longer a compile-time question, which `docs/honesty.md`
    //   says. `.cursor/rules/` stayed a literal and says why: Estigia writes no
    //   file into it.
    // - What the **host reads**, beside what this installer writes — the skills
    //   trees (derived from the same field that decides where the installer
    //   writes), plugins, commands and prompts, the Gemini and Qwen extension
    //   roots, Continue's assistant configuration, Windsurf's MCP file and
    //   workflows. Decided as a set on one principle rather than one path at a
    //   time: a path is here when a host loads it **without a person choosing it at
    //   the moment of use**. That is why the commands directories are in despite
    //   being explicitly invoked — their names and descriptions are put in front of
    //   the agent whether or not they ever are.
    // - The **per-project** twins, including this repository's own root
    //   `CLAUDE.md`. There was no principle by which `<repo>/.claude/CLAUDE.md` was
    //   a boundary and the file Claude Code loads by default was not; the line was
    //   drawn by which home-path fragment happened to carry a dotted prefix.
    //
    // The matcher grew a third anchoring rule with them — a fragment holding no
    // separator is a whole segment — and that is the matcher rather than the rule.
    // The corpus beside this list holds all three clauses: drop any
    // one of them and `the_control_surface_matcher_answers_its_corpus` reddens, so
    // recording the moved answer is what reopens the `control-surface-corpus`
    // claim.
    //
    // Reopened 2026-08-27 by registering ZCode, which brought in a file this
    // repository had named and not gated: `~/.zcode/cli/config.json` carries both
    // its `mcp.servers` block and the `hooks.enabled` flag that decides whether
    // its hooks run at all, and `~/.agents/mcp.json` is said to answer when that
    // file holds no servers of its own — unread, per `docs/honesty.md`'s ZCode
    // entry. Both are configuration a host reads to decide what an
    // agent may do, which is the principle above — the same one Windsurf's MCP
    // file was already admitted under, even though `paths_in` answers `None` for
    // that one. `~/.zcode/cli/config.json` against a
    // running ZCode CLI was then crossed too: `paths_in` now resolves it and setup writes there, which
    // only strengthens the case for gating it, and
    // `every_control_file_an_adapter_has_is_one_the_gate_measures` crosses
    // ZCode's `mcp_config` against this population. The rule did not move; two
    // paths that always satisfied it were added.
    //
    // `~/.zcode/skills` was proposed with them and is **not** here. A reviewer
    // found it ungated and it is, but the paragraph above carves the skills trees
    // out on purpose — they are derived from the `skills` field that decides where
    // the installer writes, and ZCode's is the neutral root. Admitting it as a
    // literal would have widened the rule to fit an entry, which is the move this
    // affirmation exists to catch. It is a gap, recorded in `docs/honesty.md`
    // rather than closed here.
    //
    // Re-read on 2026-08-28: moved into `CONTROL_SURFACE_POPULATION`, a `pub
    // const` immediately before `CONTROL_SURFACE`. The widening clause about
    // enforcement versus compliance inputs stayed an ordinary doc comment on
    // `CONTROL_SURFACE` itself, since it is prose about the declaration rather
    // than the declaration. Words unchanged, still true.
    //
    // Re-read: four entries for the files Pi reads to decide what to load
    // (`settings.json` at both levels, the project's `mcp.json` and `extensions/`). The rule
    // did not move; paths that always satisfied it were added.
    ("control-surface", 17443982373056176872),
    // Added 2026-09-24. The declaration above names `CONTROL_SURFACE`,
    // a list, so it held the population and not the code that matches against it:
    // three changes to `is_anchored_fragment` moved which paths the gate watches
    // while this registry stayed green, two of them narrowings. The corpus is the
    // matcher's second subject — each entry a path and the answer both roads must
    // give it — so a matcher edit that moves an answer reddens its behaviour test,
    // and recording the new answer here is what reopens this claim.
    //
    // Re-read 2026-10-04. The corpus
    // grew a relative path that begins with a fragment — held only by the
    // `starts_with` half of the anchor — and a path under an adapter-derived
    // instruction directory that no `CONTROL_SURFACE` literal names, so the
    // declaration now enumerates both seams. Words moved, still true.
    //
    // Re-read when the derivation was removed: the directory is a
    // `CONTROL_SURFACE` literal now, the corpus entry stays and the declaration
    // no longer names an adapter-derived fragment. No answer moved.
    ("control-surface-corpus", 8243348639883436307),
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
    //
    // Re-read on 2026-08-28: moved into `WRITES_A_FILE_POPULATION`, a `pub
    // const` immediately before `writes_a_file`. Words unchanged, still true.
    //
    // Re-read on 2026-10-04: the caller now hands `writes_a_file`
    // `shell::data_view`'s line, whose quoted data and here-document bodies a
    // single read-only simple command provably does not run, so a write verb
    // inside a `jq` program is no longer read as a command. `writes_a_file`
    // itself is unchanged and the fingerprint is unmoved by the change; the
    // declaration is still true — its boundary is what is *visible on the
    // command line*, and a quoted program is not on it.
    //
    // Re-read on 2026-10-05: `rg --pre` joined the reading, for the
    // flag spelling that runs a program over every file it searches. The boundary
    // is unmoved — that program is visible on the line — and a plain `rg` search
    // is still not a write.
    ("writing-shell", 8281920747523516126),
    // Read on 2026-08-04 and affirmed: the wrappers listed are the ones that
    // put another command's name after their own, and the command they run is
    // still visible on the line — which is `writing-shell`'s own boundary, not
    // an escape from it. `sudo rm -rf src` was missed by the list next door for
    // no better reason than the first word.
    //
    // Re-read on 2026-08-28: moved into `RUNS_ANOTHER_COMMAND_POPULATION`, a
    // `pub const` immediately before `RUNS_ANOTHER_COMMAND`. Words unchanged,
    // still true.
    //
    // Re-read on 2026-09-27: `script` joined the list, for BSD's
    // spelling — darwin's — which hands its command over as the words after
    // the optional typescript file rather than through the code flag the entry
    // beside it reads. The boundary is unmoved: that command is still visible
    // on the line, and a typescript file named for a write is the one-read
    // cost the declaration already accepts.
    ("running-shell", 5011553063353982520),
    // Read on 2026-08-05 and affirmed, on the round that noticed it was the one
    // boundary population in the classifier with no declaration at all — five
    // siblings in the same file carry one, and this list was added later
    // without one. Affirmed as **too-tight** knowingly: the three verbs are the
    // POSIX ways to make a control surface inert without writing to it, and the
    // declaration now names the Windows gap rather than leaving it as "a fourth
    // spelling". `icacls` is not in the list because nobody has watched it stop
    // a hook, and this crate does not add a spelling on reasoning alone.
    //
    // Re-read on 2026-08-28: moved into `NEUTRALISES_A_FILE_POPULATION`, a `pub
    // const` immediately before `NEUTRALISES_A_FILE`. Words unchanged, still
    // true.
    //
    // Re-read on 2026-10-04: the arm asked `surface_of` alone, so a
    // `chmod` on the repository's own configuration — deliberately not on the
    // control surface — was `Untouched`. The boundary sentence now names the
    // repository configuration beside the control surface, and the arm asks the
    // same `names_a_boundary` question every other shell arm asks. Same three
    // verbs, same disposition.
    ("neutralising-shell", 698627082748583322),
    // `delivery-phase` — new on 2026-08-01. Read against the constant: the five
    // spellings are the ones that land work, and the two that are absent —
    // `git push`, `gh pr create` — are absent for a reason the declaration
    // states, since gating the step that *reaches* review on being in review is
    // a deadlock. Exact equality rather than substring, so an operator's own
    // boundary is never guessed at. Too-tight, and the escape it leaves is the
    // one `irreversible-shell` already owns.
    //
    // Re-read on 2026-08-28: moved into `DELIVERS_POPULATION`, a `pub const`
    // immediately before `DELIVERS`. Words unchanged, still true.
    //
    // Re-read on 2026-10-05: the boundary sentence now says one
    // issue may land several pull requests — a chained delivery merges each
    // slice on its own — so the population counts a step and not a delivery.
    // The five spellings are unmoved.
    ("delivery-phase", 10798423924504364469),
    // `says-nothing` — new on 2026-09-07; re-read on 2026-10-03.
    // `blank-review-basis` refused an empty or whitespace-only
    // `basis`, and `record_review_finding` refused an empty `evidence` or
    // `impact` the same way, but a word wearing the same emptiness — `n/a`,
    // `TODO`, `placeholder` — satisfied all three and was recorded as the
    // stated grounds. Read against the word list, now `SAYS_NOTHING_WORDS`
    // rather than a `const` inside the function so the test iterates the one
    // list instead of retyping it: the words are the fixed floor this
    // population declares itself to be, matched on the trimmed, lowercased
    // whole value, and no minimum length was added — measured by keeping
    // `"nothing was executed"` accepted through the real MCP call in
    // `tests/pipe.rs`'s `a_verdict_names_what_it_measured_or_says_it_did_not`.
    // The subject moved from the function to the list so the fingerprint still
    // covers the words; the later change also added the never-ran evidence file to the
    // fields this population decides, and
    // `claim::tests::every_says_nothing_word_earns_its_place` exercises each
    // word the list holds.
    ("says-nothing", 875567323853761936),
    // `explained-matches-level` — new on 2026-09-07. The issue's
    // own Problem section claimed nothing cross-checks the risk classifier's
    // `reasons` against its `level`; that check has existed since `review_tier`
    // shipped, and `rg 'is not explained by its reasons' src tests` found
    // exactly one production site and nothing that had ever made it fire.
    // Investigated first, before writing anything: constructed every shape the
    // issue named worth trying — a mode-flip on a passive path, a configuration
    // path beside a passive one in both alphabetical orders, a binary stat
    // beside a configuration path in both orders, an empty-content stat beside
    // a passive one, a path that is both service-token and shell-source, a mode
    // change that spares the execute bit, a passive stat the content proof
    // contradicts alone and on both sides of a configuration path — plus a
    // brute-force sweep, since written, run and deleted, of 1,008 single-stat
    // and 96 two-stat combinations across path, mode, binary and contradiction
    // axes. None disagreed, which matches a structural proof: `classify_risk`
    // and `risk_level_from_reasons` are read from the same `reasons`
    // `derive_snapshot_risk_reasons` computes, `touches_hot_path` and
    // `derive_snapshot_risk_reasons`'s own hot-path producer read the same
    // `hot_path_risk_signals` unconditionally, `touches_configuration` is
    // always false whenever `only_passive` is true (a configuration path can
    // never be a passive candidate), and `fallback_risk_reason`'s loop
    // completes to `non_executable_only` for exactly the stats `only_passive`
    // itself accepts. No disagreement is reachable through `review_tier_answer`
    // as the two functions and their callers stand today. The guard is a
    // fail-closed backstop against a future desync between the two derivations,
    // not a live check, and this population says so rather than claiming a
    // reachable one. The check itself — until now inline in
    // `review_tier_answer` — moved into its own function,
    // `explained_matches_level`, purely so a test can hand it a `(level,
    // reasons)` pair the real computation is not known to ever produce;
    // `explained_matches_level_refuses_a_pair_the_real_path_never_produces`
    // does that and matches the refusal's own message, and deleting the `if
    // explained != level` block inside it reddened that one test with `a
    // Low-shaped reasons list beside a Medium level must refuse: ()`, restored
    // afterwards. `the_two_derivations_agree_on_a_spread_of_diff_stat_shapes`
    // pins the agreement itself over fourteen of those shapes, so an edit that
    // desyncs the two derivations reddens there on one of those fourteen and
    // cannot speak for a shape outside them.
    // Reopened: the population text now names the content scan's three reasons,
    // each carrying a signal, so both derivations still read `high` from the same reasons.
    ("explained-matches-level", 15100569489321898599),
    // `install-planned-callers` — new on 2026-10-03. `install_planned`
    // deploys adapter assets and is ungated; the guided route is safe only
    // because `guided_install` calls `lifecycle_preflight` first, and that had
    // been prose a reviewer checks rather than a test that holds it. Read
    // against the code before recording: the one production call site is
    // `guided_install` at `src/cli/mod.rs:354`, and `lifecycle_preflight` is
    // called on the line above it; the guard test
    // `every_production_caller_of_install_planned_is_preceded_by_the_lifecycle_preflight`
    // in `src/cli/tests.rs` reads the stripped source of every non-`tests.rs`
    // `src/` file and fails when a call site has no preflight in its enclosing
    // function. Deliberately too-tight: it is a line filter, so a call written
    // in a shape it does not see — inside a `#[cfg(test)]` block in a production
    // file, through a macro, a function pointer or a `use` alias — escapes it.
    // Reaffirmed for issue #36: `install_planned` now asks `skill::checkout_root` rather than
    // `here.join(".git")`; its call sites and the preflight in front of them are untouched.
    ("install-planned-callers", 10948558613944442242),
    // `tracker-transport-absent` — new on 2026-10-05, reaffirmed on
    // 2026-10-05. `Tracker` has two variants now: `Github`, whose
    // `transport()` answers `Some`, and `Linear`, whose `transport()` answers
    // `None` — so the eight arms that read that absence are reached when the
    // tracker is Linear, except `cli::inert_tracker`, which asks
    // `Tracker::adjudicates_unclaimed_writes` and so excludes a read-only tracker
    // only under `Workflow: assisted`. They are the seam that was left, and the
    // declaration binds to `Tracker`'s own variants so a further one reopens it.
    // Read against the code before recording: the eight sites are `cli::doctor`'s
    // "has no executable transport" sentence (which now names Linear's
    // assisted-only reason), `cli::inert_tracker`,
    // `harness::doctor::stale_run_pointers`'s `Skipped` row,
    // `harness::mcp::run_tool`'s tracker arm, `harness::decide_observing`'s
    // `NoTracker` aside, `harness::tracker_answer_for_pointer`,
    // `harness::queue_read` and `harness::completed_work`;
    // `harness::unclaimed::applies` and `harness::issue_requirement::in_force`
    // left the list for `Tracker::adjudicates_unclaimed_writes` (a follow-up), which holds the refusals under Linear. Deliberately
    // too-tight: a call site added before a further variant lands is not caught.
    ("tracker-transport-absent", 17163727701946764455),
    // `own-delivery-close-spelling` — new on 2026-10-05. The
    // `closed-by-own-delivery` token the transport writes into `checked[0]` and
    // the harness reads back had two spellings with nothing crossing them; they
    // now share `OWN_DELIVERY_CLOSE_PREFIX`, and this declaration binds to that const
    // so editing the token reopens the claim. Deliberately too-tight: a second
    // spelling introduced anywhere else does not move it.
    ("own-delivery-close-spelling", 8436246983933403743),
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
        // Strip comments before splitting: a comment naming `#[cfg(test)]` would
        // otherwise move the cut, and a comment naming any of `TRUNCATES` would
        // otherwise be read as an offender writing in place.
        let stripped = estigia::source_text::code_of(&source);
        // Tests plant files to be found; only the shipping code is held to this.
        let shipping = stripped.split("#[cfg(test)]").next().unwrap_or_default();
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
/// Not a lint against `Option` in fixtures generally — the last two that
/// answered one, `git_repo()` in `src/harness/doctor.rs` and `release_repo()`
/// in `src/cli/tests.rs`, are held by
/// `the_last_two_git_fixtures_cannot_answer_that_they_did_not_run` below, and
/// `docs/honesty.md` records both closures. This holds the one measured lying.
#[test]
fn the_tracker_rig_cannot_answer_that_it_did_not_run() {
    let source = estigia::source_text::code_of(&pipe_sources());
    let source = source.as_str();

    // The whole signature, not a prefix of it. A reviewer walked past
    // `contains("-> TrackerRig")` with `type TrackerRigMaybe = Option<TrackerRig>`
    // and `fn tracker_rig() -> TrackerRigMaybe {`, which contains the needle and
    // answers an option — then removed the fixture and watched 106 tests pass on
    // nothing with this guard green.
    // Exactly one: a second definition is a decoy by construction, since the
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
    // From the line, not from text that reads like it: `split_once(signature)`
    // matched the first *substring*. Exactly one line defines the rig, and the
    // body runs from it to the next line that is only a closing brace.
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
        let code = chunk.to_owned();
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
            !line.starts_with("fn ") && line.contains("tracker_rig()")
        })
        .count();
    assert!(
        callers >= 12,
        "only {callers} call site(s) of `tracker_rig` found in code, so this guard \
         is reading a spelling the suite no longer uses and would pass on a file \
         where nothing calls the rig at all"
    );
}

/// The two git fixtures that answered an `Option` hold a panic.
///
/// `git_repo()` in `src/harness/doctor.rs` and `release_repo()` in
/// `src/cli/tests.rs` returned `Option<tempfile::TempDir>` over a single
/// `git init --quiet`, and a caller spelled a missing `git` as a skip. Both now return the directory and panic instead, but the panic was held by no
/// test: reverting the fixtures and their call sites left the suite green, which
/// `AGENTS.md` calls a finding rather than a formality. This is that test.
///
/// It reads the two definitions rather than their callers. The compiler holds
/// the callers — `let Some(repo) = git_repo()` does not compile against a
/// `tempfile::TempDir` — so the signature is the load-bearing half a revert
/// trips over, and the panic is the half a compiler cannot see, which is why the
/// body is read for it too.
#[test]
fn the_last_two_git_fixtures_cannot_answer_that_they_did_not_run() {
    for (file, fixture) in [
        ("src/harness/doctor.rs", "git_repo"),
        ("src/cli/tests.rs", "release_repo"),
    ] {
        let source = std::fs::read_to_string(root().join(file))
            .unwrap_or_else(|error| panic!("{file} must be readable: {error}"));
        let source = estigia::source_text::code_of(&source);
        let needle = format!("fn {fixture}(");
        let defining: Vec<&str> = source
            .lines()
            .filter(|line| line.trim_start().starts_with(&needle))
            .collect();
        assert_eq!(
            defining.len(),
            1,
            "{file} holds {} lines that read as a definition of `{fixture}`, so this guard \
             would bind to whichever comes first: {defining:?}",
            defining.len()
        );
        assert_eq!(
            defining[0].trim(),
            format!("fn {fixture}() -> tempfile::TempDir {{"),
            "`{fixture}` in {file} no longer answers the directory itself, so a caller can be \
             handed a value meaning *did not run*"
        );
        let lines: Vec<&str> = source.lines().collect();
        let at = lines
            .iter()
            .position(|line| *line == defining[0])
            .expect("the definition line came out of this file");
        let past = lines
            .iter()
            .skip(at)
            .position(|line| line.trim() == "}")
            .expect("the fixture has an end");
        let body = lines[at..=at + past].join("\n");
        assert!(
            body.contains("panic!("),
            "`{fixture}` in {file} no longer panics when `git init --quiet` cannot spawn, so a \
             machine without git reports these tests `ok` having measured nothing"
        );
        assert!(
            body.contains("output.status.success()"),
            "`{fixture}` in {file} no longer refuses a `git init --quiet` that exits non-zero, so \
             a machine where git refuses reports these tests `ok` having measured nothing"
        );
    }
}

#[test]
fn the_envelope_guard_and_the_extractor_read_one_window() {
    let source =
        std::fs::read_to_string(root().join("build.rs")).expect("build.rs must be readable");
    let stripped = estigia::source_text::code_of(&source);

    let spelled = stripped.matches("\"pub fn envelope(\"").count();
    assert_eq!(
        spelled, 1,
        "build.rs spells the `Failure::envelope` anchor {spelled} time(s). More than one spelling \
         is how the guard and the extractor came to end their windows in different places, so both \
         must take the window from the one function that derives it"
    );

    for (reader, membership) in [
        ("fn assert_reasons_covered(", Some(".contains(&pos)")),
        ("fn extract_constructed(", None),
    ] {
        let at = stripped
            .find(reader)
            .unwrap_or_else(|| panic!("`{reader}` is not in build.rs any more"));
        let body = &stripped[at..];
        let body = &body[..body.find("\nfn ").unwrap_or(body.len())];
        assert!(
            body.contains("envelope_window("),
            "`{reader}` no longer derives its envelope window from `envelope_window`. A guard and \
             an extractor that disagree about where the window ends clear a `\"reason\":` the walk \
             never reads, which is the `transport-refused` collapse this guard exists to refuse"
        );
        if let Some(needle) = membership {
            assert!(
                body.contains(needle),
                "`assert_reasons_covered` no longer tests the `\"reason\":` position against the \
                 window it derived, so a boundary restated beside that test could widen what the \
                 guard clears without the window moving"
            );
        }
    }
}

#[test]
fn the_refusal_guard_vocabulary_is_the_harness_own() {
    let source =
        std::fs::read_to_string(root().join("build.rs")).expect("build.rs must be readable");
    let stripped = estigia::source_text::code_of(&source);
    let at = stripped
        .find("const NO_CONTINUATION_SHAPES")
        .expect("build.rs no longer declares NO_CONTINUATION_SHAPES");
    let body = &stripped[at..];
    let start = body.find("= [").expect("the declaration assigns an array") + 3;
    let end = start + body[start..].find(']').expect("the array closes");
    let mut from_build: Vec<&str> = body[start..end].split('"').skip(1).step_by(2).collect();
    from_build.sort_unstable();

    let mut from_harness: Vec<String> = [
        estigia::outcome::NoCommandReason::OperatorKnowledge,
        estigia::outcome::NoCommandReason::WorldAction,
        estigia::outcome::NoCommandReason::HumanAuthority,
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    from_harness.sort_unstable();

    assert_eq!(
        from_build,
        from_harness.iter().map(String::as_str).collect::<Vec<_>>(),
        "build.rs's refusal shapes and `NoCommandReason`'s own vocabulary have diverged. The \
         guard's closed vocabulary is the harness's, not a second one: spell it from \
         `NoCommandReason`, or add the new word to both"
    );
}
