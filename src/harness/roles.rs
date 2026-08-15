//! A sub-agent's declared tool list, made enforceable.
//!
//! # The gap this closes
//!
//! Orchestration skills ship sub-agent definitions carrying a tool allowlist —
//! a published `builder` definition declares `tools: Read, Write, Edit, Glob,
//! Grep, Bash`, and other harnesses install their phase agents the same way.
//! Every one of them then **relies on the host to honour it**. That is a
//! request, and a request is exactly what a harness exists to replace.
//!
//! Estigia already knows which sub-agent is calling: Claude Code sends
//! `agent_type` on every tool event that fires inside one. So the list somebody
//! else already wrote can be enforced rather than trusted — and nothing new has
//! to be invented, which is the point. **Estigia does not author the policy
//! here; it makes the author's policy true.**
//!
//! What it refuses to do is guess. A definition that cannot be found, or that
//! declares no list, is not an empty allowlist — see [`declared_policy`].
//!
//! # How far it reaches, which is less than the sentence above
//!
//! This runs inside `PreToolUse`, so it is offered exactly the tools the gate's
//! matcher wakes the hook for — `Edit|Write|MultiEdit|NotebookEdit|Update|Bash`
//! for Claude Code — and a call to anything else never arrives here to be
//! judged. So what it can refuse is a tool **in the matcher and not in the
//! list**, and the `builder` cited above is the measurement: of its six declared
//! tools the gate can judge three, and `Read`, `Glob` and `Grep` are never seen.
//! `WebFetch`, `WebSearch` and `Task` are not seen either, and they are usually
//! what somebody narrowing a sub-agent means.
//!
//! Widening the matcher would close it, and is the one thing the matcher exists
//! to avoid: waking this process for every `Read` is a cost paid thousands of
//! times to answer "not mine". So it is written down instead — in the README's
//! honesty section, where an operator reads it, and held by
//! `the_reach_of_the_role_gate_is_stated_where_an_operator_reads_it`. A gate
//! with a hole is still a gate; a gate whose hole nobody mentions is a lie.

use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// What a definition says about one tool.
///
/// OpenCode writes a **permission map** rather than an allowlist —
/// `permission:` with a pattern on the left and `allow`, `deny` or `ask` on the
/// right. It is the same question in a different shape, and flattening it into
/// a list would lose `ask`, which is neither of the other two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The definition permits it.
    Allow,
    /// The definition forbids it.
    Deny,
    /// The definition wants somebody asked.
    ///
    /// Estigia does **not** turn this into a denial. `ask` is the author saying
    /// a person decides, and a harness that answered for them would be taking
    /// the decision it was told to hand over.
    Ask,
}

/// The tool policy a definition declares, in whichever dialect it wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Policy {
    /// `tools: Read, Write, …` — the Claude Code dialect. Anything absent is out.
    Allowlist(Vec<String>),
    /// `permission:` — OpenCode. Patterns, most specific first.
    Permissions(Vec<(String, Verdict)>),
}

impl Policy {
    /// What this policy says about one tool.
    pub fn verdict(&self, tool: &str) -> Verdict {
        let tool = tool.trim();
        match self {
            Self::Allowlist(allowed) => {
                if may_use(allowed, tool) {
                    Verdict::Allow
                } else {
                    Verdict::Deny
                }
            }
            // An exact name beats a wildcard, whatever order they were written
            // in: `"*": deny` beside `"Read": allow` means "only Read", and
            // reading it the other way would deny the one tool the author
            // deliberately let through.
            Self::Permissions(rules) => rules
                .iter()
                .find(|(pattern, _)| pattern.eq_ignore_ascii_case(tool))
                .or_else(|| rules.iter().find(|(pattern, _)| pattern == "*"))
                .map_or(Verdict::Allow, |(_, verdict)| *verdict),
        }
    }
}

/// Whether this sub-agent may use this tool, given what its definition declared.
///
/// Case-insensitive, because the two ends are written by different people: the
/// host sends the tool name and a human typed the allowlist.
pub fn may_use(declared: &[String], tool: &str) -> bool {
    declared
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(tool.trim()))
}

/// The refusal for a sub-agent reaching past its own declared list.
///
/// Named for what happened rather than for the gate: the operator did not write
/// this list to be argued with, and the way out is to change the definition —
/// not to ask Estigia for an exception.
pub fn out_of_role(agent: &str, tool: &str, declared: &Policy) -> Refusal {
    let allowed = match declared {
        Policy::Allowlist(tools) => tools.join(", "),
        Policy::Permissions(rules) => rules
            .iter()
            .map(|(pattern, verdict)| format!("{pattern}={verdict:?}").to_lowercase())
            .collect::<Vec<_>>()
            .join(", "),
    };
    Refusal::not_started(
        "tool-outside-declared-role",
        format!("`{agent}` declares `{allowed}` and this call is `{tool}`"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!(
                "either a different sub-agent for this step, or `{tool}` added to `{agent}`'s own \
                 definition — the list is the author's, and Estigia only holds it"
            ),
        ),
    )
}

/// Whether this call has to be refused, given the definition its caller
/// declared.
///
/// Fed the definition rather than sent to find it, for the usual reason: the
/// interesting cases are a missing file and an unreadable one, and a function
/// that goes looking cannot be shown to handle either.
pub fn gate(agent: Option<&str>, tool: &str, definition: Option<&str>) -> Option<Refusal> {
    // No sub-agent: the main conversation is not a role and has no list to be
    // outside of. Treating it as one would gate every ordinary call.
    let agent = agent?;
    let policy = declared_policy(definition?)?;
    match policy.verdict(tool) {
        Verdict::Allow | Verdict::Ask => None,
        Verdict::Deny => Some(out_of_role(agent, tool, &policy)),
    }
}

/// The policy a definition declares, in either dialect.
///
/// `tools:` first, because a definition carrying both is a Claude Code file
/// somebody added a permission map to, and the key its own host reads is the
/// one that decides what actually happens.
///
/// `None` covers four different things that must not collapse into "allow
/// nothing": no definition file, a definition without a `tools:` key, one
/// declaring neither a list nor a `permission:` block, and one this parser could
/// not read. Every one of them means *this harness was not told*, and refusing
/// every tool because nobody told us would stop an agent that is behaving
/// perfectly.
///
/// An explicitly empty list — `tools: []` — is a fifth thing, and the only one
/// that is genuinely distinguishable. It is deliberately **not** distinguished:
/// after the brackets come off there is nothing left, which is what `tools:`
/// with an empty tail also leaves, and inventing a total denial out of two
/// characters is a worse mistake than not acting on them. That decision lives
/// here because this is the function that makes it.
///
/// It used to live on a second reader, `declared_tools`, that nothing in the
/// harness called: an older, weaker parser — inline commas only, no brackets, no
/// block form — kept alive by four tests and pointed at by this module's own
/// header. So the sentence explaining the live rule sat on a function that did
/// not implement it, and a test certifying the sub-agent allowlist was
/// certifying a stand-in. One rule, one reader.
pub fn declared_policy(definition: &str) -> Option<Policy> {
    let frontmatter = frontmatter(definition)?;
    if let Some(tools) = tools_line(frontmatter) {
        return Some(Policy::Allowlist(tools));
    }
    let rules = permission_rules(frontmatter);
    (!rules.is_empty()).then_some(Policy::Permissions(rules))
}

/// The text between the opening `---` and the closing one.
///
/// A byte-order mark and any blank lines before the opening `---` are stepped
/// over. Both used to make this return `None`, and `None` here is **no policy
/// at all** — the sub-agent may use every tool, because as far as this build
/// could tell its author declared nothing. A BOM is what a Windows editor
/// writes without being asked, so a definition that restricted a sub-agent
/// stopped restricting it because of a character nobody typed.
fn frontmatter(definition: &str) -> Option<&str> {
    definition
        .trim_start_matches('\u{feff}')
        .trim_start()
        .strip_prefix("---")?
        .split_once(
            "
---",
        )
        .map(|(frontmatter, _)| frontmatter)
}

/// The tools a definition declares, in either spelling YAML offers.
///
/// `tools: a, b, c` on the line, and the block form under it:
///
/// ```yaml
/// tools:
///   - Read
///   - Write
/// ```
///
/// The block form used to read as **no list at all** — the tail after `tools:`
/// is empty, so this returned `None`, and `None` is no policy: every tool
/// allowed. An author who wrote the ordinary YAML spelling of a list got a
/// sub-agent with no restrictions and nothing saying so, which is the direction
/// this crate does not let anything fail in.
///
/// Brackets and quotes come off the values for the same reason one step later:
/// `tools: [Read, Write]` parsed into `"[Read"` and `"Write]"`, which match no
/// tool name, so the list denied everything. That one fails loudly — a
/// sub-agent that can do nothing is noticed on its first call — but it is
/// still not what the author wrote.
fn tools_line(frontmatter: &str) -> Option<Vec<String>> {
    let mut lines = frontmatter.lines();
    let tail = loop {
        let line = lines.next()?;
        if let Some(tail) = line.trim().strip_prefix("tools:") {
            break tail;
        }
    };

    let clean = |tool: &str| {
        tool.trim()
            .trim_matches(['[', ']', '"', '\''])
            .trim()
            .to_owned()
    };

    let named: Vec<String> = tail
        .split(',')
        .map(clean)
        .filter(|tool| !tool.is_empty())
        .collect();
    if !named.is_empty() {
        return Some(named);
    }

    // Nothing on the line: the entries are under it, one per line, each opening
    // with a dash. A line that is not indented ends the block, the same way it
    // ends the `permission:` one.
    //
    // **A comment is not the end**, and it was. `permission_rules` below carries
    // this lesson already — *blank lines and comments are skipped, not read as
    // the end* — and it was never brought across to its neighbour, so the two
    // readers of two YAML blocks in one file disagreed about where a block
    // stops. Measured through the binary, with `Write` called by a sub-agent
    // whose list does not carry it:
    //
    // ```text
    // tools:            tools:              tools:
    //   - Read            # what it needs   # what it needs
    //   - Grep            - Read              - Read
    //                     - Grep              - Grep
    //   -> refused        -> refused        -> ALLOWED
    // ```
    //
    // An unindented comment ends the block here, so the list comes out empty,
    // and empty is `None` — no policy at all, every tool permitted. The
    // author's restriction is gone, and the direction is the one this crate
    // does not let anything fail in.
    let listed: Vec<String> = lines
        .take_while(|line| {
            let trimmed = line.trim();
            trimmed.is_empty() || trimmed.starts_with('#') || line.starts_with([' ', '\t', '-'])
        })
        .filter_map(|line| line.trim().strip_prefix('-'))
        .map(clean)
        .filter(|tool| !tool.is_empty())
        .collect();
    (!listed.is_empty()).then_some(listed)
}

/// The `permission:` block's rules, in the order they were written.
///
/// Indentation-based, because that is what YAML is here and pulling in a parser
/// for four lines would be a dependency to read somebody else's file. A line
/// that is not `key: value` under the block ends it, which is also how a
/// following top-level key ends it.
///
/// **Blank lines and comments are skipped, not read as the end.** They were
/// read as the end, and the cost ran the wrong way: a rule this parser drops
/// falls through to [`Verdict::Allow`], so one blank line between two entries
/// turned every `deny` below it into a permission. The asymmetry this crate is
/// built on says the opposite — what cannot be read must never loosen — and a
/// blank line in a YAML block is not even unreadable, it is ordinary.
fn permission_rules(frontmatter: &str) -> Vec<(String, Verdict)> {
    let mut rules = Vec::new();
    let mut inside = false;
    for line in frontmatter.lines() {
        if line.trim_start().starts_with("permission:") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        // A blank line carries no rule and does not end a YAML block. It used
        // to end this one — `"".starts_with([' ', '\t'])` is false — so every
        // rule under it was dropped, and a dropped rule falls through to
        // `Verdict::Allow`. One ordinary blank line between two entries turned
        // `"Bash": deny` into a permission.
        if line.trim().is_empty() {
            continue;
        }
        // Back at column zero: the block is over.
        if !line.starts_with([' ', '\t']) {
            break;
        }
        // Nor does a comment. Same cost as the blank line, and the same
        // direction: this gate exists to stop a sub-agent reaching past its
        // list, and the only thing that may happen to a line nobody can read is
        // that it is skipped — never that it takes the rules below it with it.
        if line.trim_start().starts_with('#') {
            continue;
        }
        let Some((pattern, verdict)) = line.trim().split_once(':') else {
            break;
        };
        let pattern = pattern.trim().trim_matches(['"', '\'']).to_owned();
        let verdict = match verdict.trim() {
            "allow" => Verdict::Allow,
            "deny" => Verdict::Deny,
            "ask" => Verdict::Ask,
            // A word this build does not know is not a denial: OpenCode may add
            // one, and refusing on it would break every run using it.
            _ => continue,
        };
        rules.push((pattern, verdict));
    }
    rules
}

/// The definition a host would load for this sub-agent.
///
/// Project first, then the operator's own directory, because that is the order
/// Claude Code resolves them in — a repository that ships its own `builder` is
/// choosing that one, and reading the home copy instead would enforce a list
/// nobody in this checkout wrote.
///
/// Returns the file's text, not a path: everything downstream is about what it
/// says, and handing back a path invites a second read that could disagree with
/// the first.
///
/// `Ok(None)` is *this sub-agent declares nothing here*, which is ordinary. The
/// error is the third answer, and it was missing: the search read each candidate
/// with `.ok()` and stepped over the ones that failed, so a definition that is
/// **there and will not open** either handed the role to a different file
/// further down the list or ran out — and running out means no policy at all,
/// which [`declared_policy`] spells out is *the sub-agent may use every tool*.
///
/// The same harm the frontmatter reader already records for a byte-order mark:
/// "a definition that restricted a sub-agent stopped restricting it because of a
/// character nobody typed". This is that, for a file nobody could read.
pub fn definition_for(
    repo_dir: &std::path::Path,
    home: Option<&std::path::Path>,
    agent: &str,
) -> Result<Option<String>, Refusal> {
    // The name arrives from the host and lands in a path. A name with a
    // separator or a parent segment would read a file outside the agents
    // directory entirely, and the list it found there would be enforced as
    // though somebody had written it for this role.
    if agent.is_empty()
        || agent.contains(['/', '\\'])
        || agent.contains("..")
        || std::path::Path::new(agent).components().count() != 1
    {
        return Ok(None);
    }
    // Project first, then the operator's — for every dialect, because a
    // repository shipping its own definition is choosing it whichever host
    // reads it.
    //
    // Both dialects are searched rather than one chosen by adapter: a machine
    // with Claude Code and OpenCode installed runs both, and the sub-agent
    // calling right now belongs to whichever one is asking. Guessing from the
    // hook's dialect would work until somebody's agent is named the same in
    // both, and then it would enforce the wrong file silently.
    let mut roots = vec![
        repo_dir.join(".claude").join("agents"),
        repo_dir.join(".opencode").join("agents"),
    ];
    if let Some(home) = home {
        roots.push(home.join(".claude").join("agents"));
        // OpenCode keeps its own under the XDG config directory, not the home
        // root — verified against a real installation. And **the** XDG config
        // directory, not `~/.config`: this was spelled by hand while `setup`
        // resolved the same root through `XDG_CONFIG_HOME`, so with that variable
        // set an OpenCode sub-agent's definition sat where this never looked.
        //
        // The failure is silent and it is a loosening. A definition that is not
        // found is `Ok(None)`, which `declared_policy` reads as *the sub-agent may
        // use every tool* — so a moved config home did not refuse the delegation,
        // it removed the allowlist from it. Measured by a reviewer of the very
        // change that moved four `CONTROL_SURFACE` entries for this variable and
        // left the enforcement road on `.config`.
        // **Both**, not whichever the variable names. Replacing the default with
        // the relocated root is the same loosening one configuration over: a
        // definition sitting at `~/.config/opencode/agents` stopped being found
        // the moment `XDG_CONFIG_HOME` pointed elsewhere, and not found is
        // `Ok(None)`, which `declared_policy` reads as *every tool allowed*. A
        // reviewer measured base ENFORCED / head NOT FOUND on exactly that input
        // — the defect this root was being fixed for, introduced by the fix.
        //
        // Searching both costs one `NotFound` stat and cannot loosen anything: a
        // definition is enforced if it is found in any root.
        roots.push(home.join(".config").join("opencode").join("agents"));
        if let Some(moved) = crate::setup::xdg_config_home() {
            roots.push(moved.join("opencode").join("agents"));
        }
    }
    for file in roots
        .into_iter()
        .map(|root| root.join(format!("{agent}.md")))
    {
        match std::fs::read_to_string(&file) {
            Ok(text) => return Ok(Some(text)),
            // Almost every candidate is absent — five roots are searched and at
            // most one holds the file.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(Refusal::not_started(
                    "agent-definition-unreadable",
                    format!("{}: {error}", file.display()),
                    Resolution::no_command(
                        NoCommandReason::OperatorKnowledge,
                        "that definition readable, or moved aside \u{2014} whatever it declares \
                         cannot be enforced while nothing can read it, and stepping over it \
                         would hand this sub-agent every tool",
                    ),
                ));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod block_tests {
    use super::{Policy, Verdict, declared_policy};

    /// An ordinary line nobody wrote a rule on does not switch the rules off.
    ///
    /// The fallthrough of a `permission:` block is `Verdict::Allow`, so every
    /// rule this parser drops is a rule that becomes a permission. It dropped
    /// them on a **blank line** — an empty line is not indented, so it read as
    /// the end of the block — and on a comment. Both are ordinary YAML, and
    /// both turned `"Bash": deny` into an allow for every entry below them.
    ///
    /// This is the declared asymmetry at the one gate that stops a sub-agent
    /// reaching past its own list: what cannot be read must never loosen.
    #[test]
    fn a_blank_line_or_a_comment_does_not_turn_off_the_rules_below_it() {
        let block = |middle: &str| {
            format!(
                "---
name: builder
permission:
  \"Read\": allow
{middle}  \"Bash\": deny
---
body"
            )
        };
        for (name, middle) in [
            ("nothing between them", ""),
            (
                "a blank line",
                "
",
            ),
            (
                "a blank line with spaces on it",
                "   
",
            ),
            (
                "a comment",
                "  # read only, please
",
            ),
            ("a comment at column zero would end the block", ""),
            (
                "both",
                "
  # read only

",
            ),
        ] {
            let policy = declared_policy(&block(middle)).expect("a policy");
            assert_eq!(
                policy.verdict("Bash"),
                Verdict::Deny,
                "{name}: a rule below it was dropped, and a dropped rule is a permission"
            );
            // And the rule above it still says what it said.
            assert_eq!(policy.verdict("Read"), Verdict::Allow, "{name}");
        }
    }

    /// The block still ends where it ends.
    ///
    /// Skipping blanks and comments must not swallow the key that follows the
    /// block: a top-level `tools:` two lines down is not a permission rule, and
    /// reading it as one would invent a pattern nobody wrote.
    #[test]
    fn the_permission_block_still_ends_at_the_next_top_level_key() {
        // A second block after this one, holding entries that read exactly like
        // permission rules. This is the fixture that can tell the difference:
        // a `description:` line is dropped anyway for having no verdict Estigia
        // knows, so it proves nothing about where the block ended.
        let definition = "---
name: builder
permission:
  \"Read\": allow

hooks:
  \"Bash\": deny
---
body";
        let policy = declared_policy(definition).expect("a policy");
        let Policy::Permissions(rules) = &policy else {
            panic!("a permission block, not an allowlist");
        };
        assert_eq!(
            rules.len(),
            1,
            "somebody else's block was read as permission rules: {rules:?}"
        );
        assert_eq!(policy.verdict("Read"), Verdict::Allow);
        // And the rule that was never in this block does not decide anything.
        assert_eq!(
            policy.verdict("Bash"),
            Verdict::Allow,
            "a rule from another block was enforced as this one's"
        );
    }

    /// A list an author wrote is a list this build reads.
    ///
    /// `declared_policy` returning `None` is not "no restriction declared" as
    /// far as the gate is concerned — it is **no restriction enforced**. So
    /// every spelling that is a real declaration has to reach it, and the ones
    /// that did not were the ordinary ones: the YAML block list, a file with a
    /// byte-order mark on it, and a blank line before the opening `---`.
    ///
    /// A Windows editor writes the BOM without being asked. That is this
    /// crate's own platform.
    #[test]
    fn every_spelling_of_a_tool_list_is_one_the_gate_enforces() {
        for (name, definition) in [
            ("on the line", "---\nname: b\ntools: Read, Write\n---\nbody"),
            (
                "the YAML block list",
                "---\nname: b\ntools:\n  - Read\n  - Write\n---\nbody",
            ),
            (
                "the block list at column zero",
                "---\nname: b\ntools:\n- Read\n- Write\n---\nbody",
            ),
            (
                "in brackets",
                "---\nname: b\ntools: [Read, Write]\n---\nbody",
            ),
            (
                "in quotes",
                "---\nname: b\ntools: \"Read, Write\"\n---\nbody",
            ),
            (
                "with a byte-order mark",
                "\u{feff}---\nname: b\ntools: Read, Write\n---\nbody",
            ),
            (
                "after a blank line",
                "\n---\nname: b\ntools: Read, Write\n---\nbody",
            ),
        ] {
            let policy = declared_policy(definition)
                .unwrap_or_else(|| panic!("{name}: a declared list reached the gate as no list"));
            assert_eq!(
                policy,
                Policy::Allowlist(vec!["Read".to_owned(), "Write".to_owned()]),
                "{name}: the names the author wrote are not the names the gate holds"
            );
            // Both halves: what was named is allowed, and what was not is not.
            assert_eq!(policy.verdict("Read"), Verdict::Allow, "{name}");
            assert_eq!(policy.verdict("Bash"), Verdict::Deny, "{name}");
        }

        // And a definition that truly declares nothing still declares nothing:
        // reading a list out of a file that has none would enforce a limit
        // nobody wrote.
        assert_eq!(declared_policy("---\nname: b\n---\nbody"), None);
        assert_eq!(declared_policy("no frontmatter here"), None);
    }
}
