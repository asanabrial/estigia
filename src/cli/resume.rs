use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::sessions::{self, Listed};
use crate::harness::{changes, session};
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::{claim, store};

#[derive(Debug)]
pub(crate) struct Claim {
    pub(crate) repo: String,
    pub(crate) issue: u64,
    pub(crate) status: &'static str,
}

#[derive(Debug)]
pub(crate) struct WorktreeState {
    pub(crate) path: PathBuf,
    pub(crate) unreadable: Option<String>,
    pub(crate) uncommitted: Option<u64>,
    pub(crate) ahead: Option<u64>,
}

impl WorktreeState {
    pub(crate) fn text(&self) -> String {
        if let Some(why) = &self.unreadable {
            return format!("{} is not readable: {why}", self.path.display());
        }
        let dirty = match self.uncommitted {
            Some(0) => "clean".to_owned(),
            Some(count) => format!("{count} uncommitted file(s)"),
            None => "uncommitted files unknown".to_owned(),
        };
        let pushed = match self.ahead {
            Some(0) => "nothing unpushed".to_owned(),
            Some(count) => format!("{count} commit(s) not pushed"),
            None => "no upstream to compare with, so the push state is unknown".to_owned(),
        };
        format!("{}: {dirty}, {pushed}", self.path.display())
    }
}

#[derive(Debug)]
pub(crate) struct Report {
    pub(crate) run_id: String,
    pub(crate) runtime: String,
    pub(crate) status: &'static str,
    pub(crate) silent_for: Option<u64>,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) claims: Vec<Claim>,
    pub(crate) worktree: Option<WorktreeState>,
    pub(crate) window_closed: bool,
}

const ALTERNATIVES: &str = "alternatives, none of them done for you: keep the row as it is; \
     `estigia release --run-id <run>` to put its issue down; hand the issue to another session \
     with `handoff`";

fn alternatives(run_id: &str) -> String {
    ALTERNATIVES.replace("<run>", run_id)
}

pub(crate) fn no_native(run_id: &str, runtime: &str, why: &str) -> Refusal {
    Refusal::not_started(
        "resume-no-native-command",
        format!(
            "{run_id} ({runtime}) has no native resume command Estigia can build: {why}; \
             nothing was resumed; {}",
            alternatives(run_id)
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the session's own program, opened by hand",
        ),
    )
}

pub(crate) fn misattributed(run_id: &str, runtime: &str, session_id: &str) -> Refusal {
    Refusal::not_started(
        "resume-runtime-misattributed",
        format!(
            "{run_id} is registered as {runtime}, but its session id {session_id} is not the \
             shape {runtime} mints, so another program may have registered as {runtime}; the \
             command was not guessed; {}",
            alternatives(run_id)
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the program that really ran this session",
        ),
    )
}

pub(crate) fn run_unknown(address: &str) -> Refusal {
    Refusal::not_started(
        "resume-run-unknown",
        format!(
            "no row of the session registry is named {address}; `estigia sessions --all` lists \
             every row"
        ),
        Resolution::run("estigia sessions --all"),
    )
}

pub(crate) fn run_ambiguous(address: &str, matched: &[Listed]) -> Refusal {
    let runs: Vec<&str> = matched.iter().map(|row| row.run_id.as_str()).collect();
    Refusal::not_started(
        "resume-run-ambiguous",
        format!(
            "{address} names {} rows ({}); nothing was resumed",
            runs.len(),
            runs.join(", ")
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "one run id from that list",
        ),
    )
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(at, found)| match at {
            8 | 13 | 18 | 23 => found == '-',
            _ => found.is_ascii_hexdigit(),
        })
}

/// The argv of the native resume command for one registered session, or the
/// refusal that says why none is built. A session id that is not the shape its
/// runtime mints is refused: the runtime field alone cannot tell a Cursor
/// session from a Claude Code one.
fn native_command(run_id: &str, runtime: &str, session_id: &str) -> Result<Vec<String>, Refusal> {
    let (words, shaped): (&[&str], bool) = match runtime {
        "claude" => (&["claude", "--resume"], is_uuid(session_id)),
        "codex" => (&["codex", "resume"], is_uuid(session_id)),
        "opencode" => (
            &["opencode", "--session"],
            session_id.starts_with("ses_")
                && session_id
                    .chars()
                    .all(|found| found.is_ascii_alphanumeric() || found == '_'),
        ),
        "pi" => {
            return Err(no_native(
                run_id,
                runtime,
                "Pi resumes from a session file whose path the registry does not record",
            ));
        }
        _ => {
            return Err(no_native(
                run_id,
                runtime,
                "no native resume command is known for this runtime",
            ));
        }
    };
    if !shaped {
        return Err(misattributed(run_id, runtime, session_id));
    }
    Ok(words
        .iter()
        .map(|word| (*word).to_owned())
        .chain([session_id.to_owned()])
        .collect())
}

pub(crate) fn worktree_state(path: &Path) -> WorktreeState {
    let tree = changes::read_tree(path.to_path_buf(), &[]);
    WorktreeState {
        path: tree.path,
        unreadable: tree.unreadable,
        uncommitted: tree.files.len().try_into().ok(),
        ahead: tree.ahead,
    }
}

fn claims_of(root: &Path, run_id: &str, now: u64) -> Result<Vec<Claim>, Refusal> {
    let unread = |failure: crate::transport::Failure| sessions::unreadable(&failure.detail());
    let Some(opened) = store::open_for_read(&store::beside(root)).map_err(unread)? else {
        return Ok(Vec::new());
    };
    let stamp = session::stamp_of(now);
    let mut claims = Vec::new();
    for (repo, issue) in opened.claims_marked_by(run_id).map_err(unread)? {
        let holding = claim::holding(&opened.timeline(&repo, issue).map_err(unread)?, &stamp);
        let by_run = |events: &[crate::transport::ownership::Event]| {
            events.iter().any(|event| event.run_id == run_id)
        };
        let status = if by_run(&holding.live) {
            "live"
        } else if by_run(&holding.stale) {
            "lapsed"
        } else {
            continue;
        };
        claims.push(Claim {
            repo: format!("{}/{}/{}", repo.host, repo.owner, repo.name),
            issue,
            status,
        });
    }
    Ok(claims)
}

/// What resuming `address` would take: the command, where to run it, the claim
/// and the worktree as they stand. Reads only; nothing is started.
pub(crate) fn report(
    root: &Path,
    address: &str,
    now: u64,
    window_closed: bool,
) -> Result<Report, Refusal> {
    let rows = sessions::entries(root)?;
    let matched: Vec<Listed> = rows
        .into_iter()
        .filter(|row| row.run_id == address || row.name.as_deref() == Some(address))
        .collect();
    let row = match matched.as_slice() {
        [only] => only,
        [] => return Err(run_unknown(address)),
        _ => return Err(run_ambiguous(address, &matched)),
    };
    let Some(session_id) = row.session_id.as_deref() else {
        return Err(no_native(
            &row.run_id,
            &row.runtime,
            "the registry holds no session id for it, only a run pointer",
        ));
    };
    let command = native_command(&row.run_id, &row.runtime, session_id)?;
    let worktree = row.worktree.as_deref().map(PathBuf::from);
    let cwd = worktree
        .clone()
        .or_else(|| row.repo_dir.as_deref().map(PathBuf::from));
    Ok(Report {
        claims: claims_of(root, &row.run_id, now)?,
        worktree: worktree.as_deref().map(worktree_state),
        run_id: row.run_id.clone(),
        runtime: row.runtime.clone(),
        status: row.status,
        silent_for: row.silent_for,
        command,
        cwd,
        window_closed,
    })
}

impl Report {
    pub(crate) fn text(&self) -> String {
        let mut lines = vec![format!(
            "{} ({}) is {}{}",
            self.run_id,
            self.runtime,
            self.status,
            self.silent_for.map_or_else(String::new, |seconds| format!(
                ", seen {}",
                session::age(seconds)
            ))
        )];
        lines.push(format!(
            "run in: {}",
            self.cwd
                .as_deref()
                .map_or_else(|| "unknown".to_owned(), |dir| dir.display().to_string())
        ));
        lines.push(format!("command: {}", self.command.join(" ")));
        if self.claims.is_empty() {
            lines.push(
                "claim: none live or lapsed on record; it was released, taken over, or never held"
                    .to_owned(),
            );
        }
        for held in &self.claims {
            lines.push(format!(
                "claim: {}#{} is {}",
                held.repo, held.issue, held.status
            ));
        }
        lines.push(match &self.worktree {
            Some(state) => format!("worktree: {}", state.text()),
            None => "worktree: unknown, no run pointer names one".to_owned(),
        });
        lines.push(
            "liveness is not known: do not resume a session that is still open in another \
             terminal"
                .to_owned(),
        );
        lines.push(
            "a resumed session does not get its claim back by itself: call `claim` again"
                .to_owned(),
        );
        if self.window_closed {
            lines.push(
                "the git operation window is closed: resuming works, but the pushes and \
                 commits that follow are refused until it opens"
                    .to_owned(),
            );
        }
        lines.push(alternatives(&self.run_id));
        lines.join("\n")
    }

    pub(crate) fn json(&self) -> Value {
        json!({
            "run_id": self.run_id,
            "runtime": self.runtime,
            "status": self.status,
            "silent_for": self.silent_for,
            "command": self.command,
            "cwd": self.cwd.as_ref().map(|dir| dir.display().to_string()),
            "claims": self.claims.iter().map(|held| json!({
                "repo": held.repo, "issue": held.issue, "status": held.status,
            })).collect::<Vec<_>>(),
            "worktree": self.worktree.as_ref().map(|state| json!({
                "path": state.path.display().to_string(),
                "unreadable": state.unreadable,
                "uncommitted": state.uncommitted,
                "ahead": state.ahead,
            })),
            "liveness": "unknown",
            "window_closed": self.window_closed,
        })
    }
}

pub(super) fn resume(address: &str, json: bool) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let repo_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let context =
        crate::transport::Context::live(crate::harness::control_surface(), repo_dir, None);
    let closed = matches!(
        crate::transport::window::state(&context),
        crate::transport::window::Mode::Closed(_)
    );
    let now = session::now_seconds().ok_or_else(|| {
        Refusal::unavailable(
            "clock-unreadable",
            "the clock could not be read, so a claim cannot be called live or lapsed",
            Resolution::no_command(NoCommandReason::WorldAction, "a working system clock"),
        )
    })?;
    let answer = report(&root, address, now, closed)?;
    if json {
        super::print_json(&answer.json());
    } else {
        say!("{}", answer.text());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::harness::session;
    use crate::transport::store::{self, Repo, SessionSeen, Store};

    const OPENCODE: &str = "ses_0198fe1c2b7a7d4e9f01abcdef";
    const UUID: &str = "0198fe1c-2b7a-7d4e-9f01-abcdef012345";

    fn repo() -> Repo {
        Repo {
            host: "github.com".to_owned(),
            owner: "asanabrial".to_owned(),
            name: "estigia".to_owned(),
        }
    }

    fn rig() -> (tempfile::TempDir, PathBuf, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let root = directory.path().join("runs");
        std::fs::create_dir(&root).expect("a state root");
        let store = Store::open(&store::beside(&root)).expect("the store opens");
        (directory, root, store)
    }

    fn register(store: &Store, runtime: &str, session_id: &str, seen: u64) -> String {
        let run_id = session::run_id(runtime, session_id);
        store
            .session_seen(
                &SessionSeen {
                    run_id: &run_id,
                    runtime,
                    session_id,
                    repo_dir: Some("/work/estigia"),
                    wake: None,
                    pane: None,
                },
                seen,
            )
            .expect("the session registers");
        run_id
    }

    fn claim(id: &str, run_id: &str, horizon: &str) -> serde_json::Value {
        serde_json::json!({
            "body": "Body.\n\n<!-- issue-flow: analysis identity=thing-1 -->\n",
            "labels": [{ "name": "status:in-progress" }],
            "comments": [{
                "id": id,
                "createdAt": "2020-01-01T00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!(
                    "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=opencode \
                     horizon={horizon} op-id=0123456789abcdef0123456789abcdef -->\n"
                ),
            }],
        })
    }

    fn now() -> u64 {
        session::now_seconds().expect("a clock")
    }

    fn argv(report: &Report) -> Vec<&str> {
        report.command.iter().map(String::as_str).collect()
    }

    fn git(at: &Path, arguments: &[&str]) {
        let output = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(arguments)
            .current_dir(at)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {arguments:?}: {output:?}");
    }

    #[test]
    fn each_runtime_gets_its_own_native_command_and_nothing_else_is_guessed() {
        assert_eq!(
            native_command("x", "opencode", OPENCODE).expect("opencode"),
            ["opencode", "--session", OPENCODE]
        );
        assert_eq!(
            native_command("x", "claude", UUID).expect("claude"),
            ["claude", "--resume", UUID]
        );
        assert_eq!(
            native_command("x", "codex", UUID).expect("codex"),
            ["codex", "resume", UUID]
        );
        for (runtime, session_id, code) in [
            ("pi", "019a-pi", "resume-no-native-command"),
            ("cursor", UUID, "resume-no-native-command"),
            ("zcode", UUID, "resume-no-native-command"),
            (
                "claude",
                "cursor-conversation-7",
                "resume-runtime-misattributed",
            ),
            ("opencode", UUID, "resume-runtime-misattributed"),
            ("codex", OPENCODE, "resume-runtime-misattributed"),
        ] {
            let refusal = native_command("x", runtime, session_id).expect_err(runtime);
            assert_eq!(refusal.code, code, "{runtime} {session_id}");
            assert!(
                refusal.message.contains("release --run-id"),
                "a refusal offers the alternatives: {}",
                refusal.message
            );
        }
    }

    #[test]
    fn a_stale_opencode_run_prints_its_command_and_the_worktree_it_ran_in() {
        let (directory, root, store) = rig();
        let run_id = register(&store, "opencode", OPENCODE, now() - 7 * 3600);
        let worktree = directory.path().join("tree");
        std::fs::create_dir(&worktree).expect("a worktree");
        let mut pointer = session::load(&root, &run_id);
        pointer.issue = Some(crate::identity::Identity::Number(7));
        pointer.worktree = Some(worktree.clone());
        session::store(&root, &pointer).expect("the pointer is stored");

        let report = report(&root, &run_id, now(), false).expect("a report");

        assert_eq!(argv(&report), ["opencode", "--session", OPENCODE]);
        assert_eq!(report.cwd.as_deref(), Some(worktree.as_path()));
        assert_eq!(report.status, "stale");
        assert!(report.text().contains("opencode --session ses_0198"));
        assert!(report.text().contains("call `claim`"));
    }

    #[test]
    fn an_ended_run_with_no_pointer_still_finds_its_claim_and_says_live_or_lapsed() {
        let (_directory, root, mut store) = rig();
        let live = register(&store, "opencode", OPENCODE, now() - 100);
        let lapsed_session = "ses_0198fe1c2b7a7d4e9f01fedcba";
        let lapsed = register(&store, "opencode", lapsed_session, now() - 100);
        store
            .session_ended(&live, Some("exit"), now())
            .expect("ends");
        store
            .session_ended(&lapsed, Some("exit"), now())
            .expect("ends");
        store
            .import(
                &repo(),
                7,
                &claim("IC_1", &live, "2099-01-01T00:00Z"),
                "2026-01-01T00:00Z",
            )
            .expect("a live claim imports");
        store
            .import(
                &repo(),
                8,
                &claim("IC_2", &lapsed, "2020-01-02T00:00Z"),
                "2026-01-01T00:00Z",
            )
            .expect("a lapsed claim imports");

        let first = report(&root, &live, now(), false).expect("a report");
        let second = report(&root, &lapsed, now(), false).expect("a report");

        assert_eq!(first.status, "ended");
        assert_eq!(first.claims.len(), 1);
        assert_eq!((first.claims[0].issue, first.claims[0].status), (7, "live"));
        assert_eq!(
            (second.claims[0].issue, second.claims[0].status),
            (8, "lapsed")
        );
        assert!(first.text().contains("#7"));
        assert!(second.text().contains("lapsed"));
    }

    #[test]
    fn the_worktree_reports_what_is_dirty_and_what_is_not_pushed() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let remote = directory.path().join("remote.git");
        let tree = directory.path().join("tree");
        std::fs::create_dir(&remote).expect("remote");
        git(&remote, &["init", "--bare", "-b", "main"]);
        git(
            directory.path(),
            &["clone", remote.to_str().expect("utf-8"), "tree"],
        );
        std::fs::write(tree.join("a"), "a").expect("a file");
        git(&tree, &["add", "a"]);
        git(&tree, &["commit", "-m", "first"]);
        git(&tree, &["push", "-u", "origin", "HEAD:main"]);
        git(&tree, &["fetch", "origin"]);
        git(&tree, &["branch", "--set-upstream-to=origin/main"]);
        std::fs::write(tree.join("b"), "b").expect("a file");
        git(&tree, &["add", "b"]);
        git(&tree, &["commit", "-m", "second"]);
        std::fs::write(tree.join("c"), "c").expect("a file");

        let state = worktree_state(&tree);

        assert_eq!(state.uncommitted, Some(1));
        assert_eq!(state.ahead, Some(1));
        assert!(state.text().contains("1 uncommitted"));
        assert!(state.text().contains("1 commit(s) not pushed"));
    }

    #[test]
    fn a_run_that_is_not_in_the_registry_is_refused_and_a_misattributed_one_is_not_guessed() {
        let (_directory, root, store) = rig();
        let cursor = register(&store, "claude", "cursor-conversation-7", now());

        let unknown = report(&root, "opencode-nothing", now(), false).expect_err("unknown");
        let guessed = report(&root, &cursor, now(), false).expect_err("misattributed");

        assert_eq!(unknown.code, "resume-run-unknown");
        assert_eq!(guessed.code, "resume-runtime-misattributed");
    }
}
