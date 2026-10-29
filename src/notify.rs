//! The OS notification for a moment a run needs a person.
//!
//! Emitted by the binary at the store write that records the need, so it is
//! the same on every runtime. Best effort and never a guarantee: a notifier
//! that exits 0 has been *handed* the text, which says nothing about anybody
//! seeing it, and the text is never an approval. See
//! `docs/honesty.md#the-needs-person-record`.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::Notifications;
use crate::transport::Failure;
use crate::transport::store::{Need, Store};

/// What the record says once the notifier was handed the text.
pub const HANDED: &str = "handed to the OS";
/// What the record says while a notification is being handed over.
pub const HANDING: &str = "handing to the OS";
/// What the record says when no notifier exists on this machine.
pub const NO_NOTIFIER: &str = "not delivered: no notifier";
/// What the record says when the channel is off.
pub const CHANNEL_OFF: &str = "not delivered: channel off";
/// What the record says when quiet hours held the notification back.
pub const QUIET: &str = "held: quiet hours";
/// What the record says when the hourly cap held the notification back.
pub const CAPPED: &str = "held: hourly cap";

/// What the record says when the notifier failed or did not answer in time.
pub const NOTIFIER_FAILED: &str = "not delivered: notifier failed";

const BOUND: Duration = Duration::from_secs(2);
const HOUR: u64 = 3600;
const TOKEN_SHAPED: usize = 32;

/// The words a notification carries: only repo, issue, kind and run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// The notification's title.
    pub title: String,
    /// The notification's one line of body.
    pub body: String,
}

fn plain(field: &str) -> String {
    let tail: Vec<&str> = field
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    let kept = tail[tail.len().saturating_sub(2)..].join("/");
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() >= TOKEN_SHAPED {
            out.push_str("redacted");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in kept.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            run.push(c);
            continue;
        }
        flush(&mut run, &mut out);
        out.push(if c == '.' || c == '/' { c } else { '_' });
    }
    flush(&mut run, &mut out);
    out.chars().take(64).collect()
}

/// The text for one need. Nothing but the four fields reaches it, and each is
/// reduced to a safe alphabet with token-shaped runs removed.
pub(crate) fn text(need: &Need<'_>) -> Text {
    let issue = need
        .issue
        .map_or(String::new(), |issue| format!("#{issue}"));
    let run = need
        .run_id
        .map_or(String::new(), |run| format!(" run {}", plain(run)));
    Text {
        title: "Estigia: a person is needed".to_owned(),
        body: format!(
            "{} {}{issue}{run}. Not an approval or consent: answer in the session.",
            plain(need.kind),
            plain(need.repo)
        ),
    }
}

/// The operating system a notifier is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// `osascript`.
    MacOs,
    /// `notify-send`.
    Linux,
    /// A PowerShell toast.
    Windows,
    /// No notifier is known.
    Other,
}

impl Platform {
    /// The platform this binary was built for.
    pub fn here() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else if cfg!(unix) {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// One program and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The program, found on `PATH`.
    pub program: &'static str,
    /// Its arguments.
    pub args: Vec<String>,
}

/// The command that hands `text` to this platform's notifier, when the program
/// it needs is present. No action buttons are ever offered.
pub fn invocation(
    platform: Platform,
    text: &Text,
    present: &dyn Fn(&str) -> bool,
) -> Option<Invocation> {
    let (program, args) = match platform {
        Platform::MacOs => (
            "osascript",
            vec![
                "-e".to_owned(),
                format!(
                    "display notification \"{}\" with title \"{}\"",
                    text.body, text.title
                ),
            ],
        ),
        Platform::Linux => (
            "notify-send",
            vec![
                "--app-name=Estigia".to_owned(),
                text.title.clone(),
                text.body.clone(),
            ],
        ),
        Platform::Windows => (
            "powershell",
            vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                format!(
                    "[Windows.UI.Notifications.ToastNotificationManager, \
                     Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
                     $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent('ToastText02'); \
                     $n = $t.GetElementsByTagName('text'); \
                     $n.Item(0).AppendChild($t.CreateTextNode('{}')) | Out-Null; \
                     $n.Item(1).AppendChild($t.CreateTextNode('{}')) | Out-Null; \
                     [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Estigia').Show([Windows.UI.Notifications.ToastNotification]::new($t))",
                    text.title, text.body
                ),
            ],
        ),
        Platform::Other => return None,
    };
    present(program).then_some(Invocation { program, args })
}

/// How a hand-over ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The notifier exited 0: handed to the OS, not seen by anybody.
    Handed,
    /// No notifier exists on this machine.
    NoNotifier,
    /// The notifier failed or did not answer in time.
    Failed(String),
}

/// What turns a [`Text`] into a notification.
pub trait Notifier {
    /// Whether a notifier exists, asked before the hourly cap is spent.
    fn available(&self, text: &Text) -> bool;
    /// Hands the text over, waiting a short bound at most.
    fn send(&self, text: &Text) -> Outcome;
}

fn on_path(program: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        [program.to_owned(), format!("{program}.exe")]
            .iter()
            .any(|name| Path::new(&dir).join(name).is_file())
    })
}

/// The real notifier of this machine.
pub struct Os;

impl Notifier for Os {
    fn available(&self, text: &Text) -> bool {
        invocation(Platform::here(), text, &on_path).is_some()
    }

    fn send(&self, text: &Text) -> Outcome {
        let Some(call) = invocation(Platform::here(), text, &on_path) else {
            return Outcome::NoNotifier;
        };
        let mut child = match Command::new(call.program)
            .args(&call.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => return Outcome::Failed(format!("could not start: {}", error.kind())),
        };
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Outcome::Handed,
                Ok(Some(status)) => {
                    return Outcome::Failed(format!(
                        "exited {}",
                        status
                            .code()
                            .map_or("by signal".to_owned(), |c| c.to_string())
                    ));
                }
                Ok(None) if started.elapsed() < BOUND => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Outcome::Failed("did not answer in time".to_owned());
                }
            }
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static STAND_IN: std::cell::RefCell<Option<std::rc::Rc<dyn Notifier>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct Absent;

#[cfg(test)]
impl Notifier for Absent {
    fn available(&self, _: &Text) -> bool {
        false
    }
    fn send(&self, _: &Text) -> Outcome {
        Outcome::NoNotifier
    }
}

/// Runs `run` with `notifier` standing in for the OS on this thread. Tests
/// never reach the real notifier: without a stand-in there is none.
#[cfg(test)]
pub fn with_notifier<T>(notifier: std::rc::Rc<dyn Notifier>, run: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            STAND_IN.with(|cell| *cell.borrow_mut() = None);
        }
    }
    STAND_IN.with(|cell| *cell.borrow_mut() = Some(notifier));
    let _reset = Reset;
    run()
}

fn notifier() -> std::rc::Rc<dyn Notifier> {
    #[cfg(test)]
    {
        STAND_IN
            .with(|cell| cell.borrow().clone())
            .unwrap_or_else(|| std::rc::Rc::new(Absent))
    }
    #[cfg(not(test))]
    {
        std::rc::Rc::new(Os)
    }
}

/// Records a need and, the first time, hands it to the OS.
///
/// Answers whether this call wrote the record. The notification is held back
/// inside quiet hours and past the hourly cap, which is counted in the store so
/// that two processes share it; the record says what happened to it.
pub(crate) fn record(
    store: &Store,
    need: &Need<'_>,
    policy: &Notifications,
    now: u64,
) -> Result<bool, Failure> {
    if !store.need_recorded(need, now)? {
        return Ok(false);
    }
    let said = announce(store, need, policy, now)?;
    store.delivery_set(need, said)?;
    Ok(true)
}

fn announce(
    store: &Store,
    need: &Need<'_>,
    policy: &Notifications,
    now: u64,
) -> Result<&'static str, Failure> {
    if !policy.os {
        return Ok(CHANNEL_OFF);
    }
    let quiet = policy.quiet.enabled()
        && crate::harness::operation_window::now_minutes()
            .is_none_or(|minutes| policy.quiet.permits(minutes));
    if quiet {
        return Ok(QUIET);
    }
    let text = text(need);
    let notifier = notifier();
    if !notifier.available(&text) {
        return Ok(NO_NOTIFIER);
    }
    if !store.delivery_claimed(need, now.saturating_sub(HOUR), policy.cap)? {
        return Ok(CAPPED);
    }
    Ok(match notifier.send(&text) {
        Outcome::Handed => HANDED,
        Outcome::NoNotifier => NO_NOTIFIER,
        Outcome::Failed(_) => NOTIFIER_FAILED,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    struct Recording {
        sent: RefCell<Vec<Text>>,
        present: bool,
        outcome: Outcome,
    }

    impl Recording {
        fn handing() -> Rc<Self> {
            Rc::new(Self {
                sent: RefCell::new(Vec::new()),
                present: true,
                outcome: Outcome::Handed,
            })
        }
    }

    impl Notifier for Recording {
        fn available(&self, _: &Text) -> bool {
            self.present
        }
        fn send(&self, text: &Text) -> Outcome {
            self.sent.borrow_mut().push(text.clone());
            self.outcome.clone()
        }
    }

    fn need<'a>(kind: &'a str, key: &'a str, repo: &'a str) -> Need<'a> {
        Need {
            kind,
            repo,
            issue: Some(7),
            run_id: Some("claude-abcd1234"),
            dedup_key: key,
        }
    }

    fn rows(store: &Store) -> Vec<(String, String)> {
        let mut rows = store.needs_person(50).expect("reads");
        rows.reverse();
        rows.into_iter().map(|r| (r.kind, r.delivery)).collect()
    }

    fn noon() -> Option<u16> {
        Some(12 * 60)
    }

    fn eleven_at_night() -> Option<u16> {
        Some(23 * 60)
    }

    fn written(
        policy: &str,
        notifier: &Rc<Recording>,
        needs: &[(&str, &str, u64)],
    ) -> Vec<(String, String)> {
        let directory = tempfile::tempdir().expect("a directory");
        let store = Store::open(&directory.path().join("state.db")).expect("opens");
        let policy = Notifications::parse(policy).expect("a policy");
        crate::harness::operation_window::with_now_override(Some(noon), || {
            with_notifier(notifier.clone(), || {
                for (kind, key, now) in needs {
                    record(&store, &need(kind, key, "github.com/o/r"), &policy, *now)
                        .expect("records");
                }
            })
        });
        rows(&store)
    }

    #[test]
    fn the_text_carries_repo_issue_kind_and_run_and_says_it_is_not_an_approval() {
        let said = text(&need("escalated", "k", "github.com/asanabrial/estigia"));
        assert_eq!(
            said.body,
            "escalated asanabrial/estigia#7 run claude-abcd1234. Not an approval or consent: \
             answer in the session."
        );
        assert!(!said.title.contains("approve"));
    }

    #[test]
    fn nothing_token_shaped_or_free_form_reaches_the_text() {
        let secret = "ghp_0123456789abcdefghijklmnopqrstuvwxyz0123";
        let hostile = format!("/home/a\"b/{secret}/pro'j\\x;rm");
        let said = text(&Need {
            kind: "ask",
            repo: &hostile,
            issue: None,
            run_id: Some(secret),
            dedup_key: "k",
        });
        let all = format!("{} {}", said.title, said.body);
        assert!(!all.contains("ghp_"), "{all}");
        assert!(!all.contains("0123456789abcdef"), "{all}");
        for quote in ['"', '\'', ';', '\\', '$', '`'] {
            assert!(!said.body.contains(quote), "{quote}: {}", said.body);
        }
    }

    #[test]
    fn each_platform_hands_the_text_to_its_own_notifier_and_offers_no_buttons() {
        let said = text(&need("blocked", "k", "github.com/o/r"));
        let yes = |_: &str| true;
        let mac = invocation(Platform::MacOs, &said, &yes).expect("osascript");
        assert_eq!((mac.program, mac.args[0].as_str()), ("osascript", "-e"));
        assert!(mac.args[1].starts_with("display notification \""));
        assert!(mac.args[1].contains(&said.body));
        let linux = invocation(Platform::Linux, &said, &yes).expect("notify-send");
        assert_eq!(linux.program, "notify-send");
        assert_eq!(linux.args[1..], [said.title.clone(), said.body.clone()]);
        let windows = invocation(Platform::Windows, &said, &yes).expect("powershell");
        assert_eq!(windows.program, "powershell");
        assert!(windows.args[3].contains("ToastNotification"));
        assert!(windows.args[3].contains(&said.body));
        for call in [&mac, &linux, &windows] {
            assert!(
                !call.args.join(" ").to_lowercase().contains("action"),
                "{call:?}"
            );
        }
    }

    #[test]
    fn a_missing_program_or_an_unknown_platform_has_no_notifier() {
        let said = text(&need("blocked", "k", "github.com/o/r"));
        assert_eq!(invocation(Platform::Linux, &said, &|_| false), None);
        assert_eq!(invocation(Platform::Other, &said, &|_| true), None);
    }

    #[test]
    fn a_need_is_handed_to_the_os_once_and_a_repeat_hands_nothing() {
        let notifier = Recording::handing();
        let rows = written(
            "os",
            &notifier,
            &[("escalated", "a", 1000), ("escalated", "a", 1001)],
        );
        assert_eq!(rows, [("escalated".to_owned(), HANDED.to_owned())]);
        assert_eq!(notifier.sent.borrow().len(), 1);
    }

    #[test]
    fn with_the_channel_off_the_record_is_written_and_nothing_is_handed_over() {
        let notifier = Recording::handing();
        let rows = written("off", &notifier, &[("blocked", "a", 1000)]);
        assert_eq!(rows, [("blocked".to_owned(), CHANNEL_OFF.to_owned())]);
        assert!(notifier.sent.borrow().is_empty());
    }

    #[test]
    fn quiet_hours_hold_the_notification_back_and_the_record_is_still_written() {
        let notifier = Recording::handing();
        let directory = tempfile::tempdir().expect("a directory");
        let store = Store::open(&directory.path().join("state.db")).expect("opens");
        let policy = Notifications::parse("os quiet 22:00-07:00").expect("a policy");
        let held = need("blocked", "a", "github.com/o/r");
        let open = need("blocked", "b", "github.com/o/r");
        with_notifier(notifier.clone(), || {
            crate::harness::operation_window::with_now_override(Some(eleven_at_night), || {
                record(&store, &held, &policy, 1000).expect("records");
            });
            crate::harness::operation_window::with_now_override(Some(noon), || {
                record(&store, &open, &policy, 2000).expect("records");
            });
        });
        assert_eq!(
            rows(&store),
            [
                ("blocked".to_owned(), QUIET.to_owned()),
                ("blocked".to_owned(), HANDED.to_owned())
            ]
        );
        assert_eq!(notifier.sent.borrow().len(), 1);
    }

    #[test]
    fn the_hourly_cap_is_counted_in_the_store_and_frees_after_an_hour() {
        let notifier = Recording::handing();
        let rows = written(
            "os cap 2",
            &notifier,
            &[
                ("blocked", "a", 1000),
                ("blocked", "b", 1100),
                ("blocked", "c", 1200),
                ("blocked", "d", 1000 + HOUR + 1),
            ],
        );
        let said: Vec<&str> = rows.iter().map(|(_, d)| d.as_str()).collect();
        assert_eq!(said, [HANDED, HANDED, CAPPED, HANDED]);
        assert_eq!(notifier.sent.borrow().len(), 3);
    }

    #[test]
    fn two_handles_on_one_store_share_the_cap() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let (first, second) = (
            Store::open(&path).expect("opens"),
            Store::open(&path).expect("opens"),
        );
        let policy = Notifications::parse("os cap 1").expect("a policy");
        let notifier = Recording::handing();
        with_notifier(notifier.clone(), || {
            crate::harness::operation_window::with_now_override(Some(noon), || {
                record(&first, &need("blocked", "a", "r"), &policy, 1000).expect("records");
                record(&second, &need("blocked", "b", "r"), &policy, 1001).expect("records");
            });
        });
        assert_eq!(notifier.sent.borrow().len(), 1);
        assert_eq!(rows(&second)[1].1, CAPPED);
    }

    #[test]
    fn no_notifier_and_a_failing_notifier_are_recorded_for_what_they_are() {
        let missing = Rc::new(Recording {
            sent: RefCell::new(Vec::new()),
            present: false,
            outcome: Outcome::Handed,
        });
        assert_eq!(
            written("os", &missing, &[("blocked", "a", 1)])[0].1,
            NO_NOTIFIER
        );
        assert!(missing.sent.borrow().is_empty());
        let failing = Rc::new(Recording {
            sent: RefCell::new(Vec::new()),
            present: true,
            outcome: Outcome::Failed("exited 1".to_owned()),
        });
        assert_eq!(
            written("os", &failing, &[("blocked", "a", 1)])[0].1,
            NOTIFIER_FAILED
        );
    }

    #[test]
    fn a_store_that_cannot_be_written_is_an_error_and_not_none() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let store = Store::open(&path).expect("opens");
        rusqlite::Connection::open(&path)
            .expect("a second handle")
            .execute_batch("DROP TABLE needs_person")
            .expect("drops");
        let policy = Notifications::default();
        assert!(record(&store, &need("blocked", "a", "r"), &policy, 1).is_err());
    }

    #[test]
    fn the_notification_policy_reads_back_as_written() {
        for value in [
            "off",
            "os",
            "os cap 3",
            "os quiet 22:00-07:00",
            "os quiet mon-fri 22:00-07:00 cap 2",
        ] {
            let policy = Notifications::parse(value).expect(value);
            assert_eq!(policy.as_value(), value);
        }
        for value in [
            "on",
            "os cap 0",
            "os cap",
            "os quiet",
            "os quiet nonsense",
            "os x",
        ] {
            assert_eq!(Notifications::parse(value), None, "{value}");
        }
    }
}
