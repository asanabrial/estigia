//! The repository's daily range for commits, pushes, and branch creation.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use estigia::config::{Config, GitOperationWindow, SETTINGS, Setting};
use estigia::harness::operation_window as window;
use estigia::harness::{Action, classify};
use serde_json::json;

fn enabled(value: &str) -> GitOperationWindow {
    GitOperationWindow::parse(value).expect("a usable window value")
}

#[test]
fn the_window_is_a_single_disabled_by_default_row() {
    assert!(
        SETTINGS.contains(&Setting::GitOperationWindow),
        "no single repository row carries the schedule"
    );
    assert_eq!(
        Config::default().git_operation_window,
        GitOperationWindow::Disabled,
        "an unconfigured repository does not start restricted"
    );
    assert_eq!(
        Setting::GitOperationWindow.scope(),
        estigia::config::Scope::Everywhere,
        "a per-agent schedule would differ by reader while the gate is one"
    );
}

#[test]
fn enabled_forms_parse_and_render_back() {
    for value in [
        "enabled 09:00-17:00",
        "enabled 22:00-06:00",
        "enabled 09:00-17:00 inverted",
        "enabled 22:00-06:00 inverted",
    ] {
        let parsed = enabled(value);
        assert!(parsed.enabled(), "{value:?} did not read as enabled");
        assert_eq!(
            parsed.as_value(),
            value,
            "{value:?} did not round-trip through rendering"
        );
        let mut config = Config::default();
        Setting::GitOperationWindow
            .apply(&mut config, value)
            .unwrap_or_else(|refusal| panic!("{value:?} was refused: {refusal}"));
        assert_eq!(
            Setting::GitOperationWindow.value_of(&config),
            value,
            "{value:?} did not round-trip through the setting"
        );
    }
}

#[test]
fn equal_endpoints_and_malformed_values_are_invalid() {
    for bad in [
        "enabled 09:00-09:00",
        "enabled 00:00-00:00",
        "enabled 25:00-06:00",
        "enabled 09:60-10:00",
        "enabled 09:00",
        "enabled -17:00",
        "enabled 09:00-17:00 sideways",
        "nine to five",
    ] {
        assert!(
            GitOperationWindow::parse(bad).is_none(),
            "{bad:?} parsed as a schedule"
        );
        let mut config = Config::default();
        assert!(
            Setting::GitOperationWindow.apply(&mut config, bad).is_err(),
            "{bad:?} was accepted by the setter"
        );
    }
}

#[test]
fn intervals_are_half_open_and_midnight_crossing() {
    let day = enabled("enabled 09:00-17:00");
    assert!(day.permits(9 * 60), "the start is inclusive");
    assert!(!day.permits(17 * 60), "the end is exclusive");
    assert!(day.permits(12 * 60), "midday is inside");
    assert!(!day.permits(8 * 60 + 59), "before the start is outside");

    let night = enabled("enabled 22:00-06:00");
    assert!(night.permits(22 * 60), "overnight start is inclusive");
    assert!(night.permits(2 * 60), "after midnight is inside");
    assert!(!night.permits(6 * 60), "overnight end is exclusive");
    assert!(!night.permits(12 * 60), "midday is outside overnight");
}

#[test]
fn inversion_permits_the_exact_complement() {
    let normal = enabled("enabled 09:00-17:00");
    let inverted = enabled("enabled 09:00-17:00 inverted");
    for minutes in (0..1440).step_by(37) {
        assert_ne!(
            normal.permits(minutes),
            inverted.permits(minutes),
            "inversion disagrees nowhere at {minutes}"
        );
    }
    assert!(
        !inverted.permits(12 * 60),
        "midday is outside when inverted"
    );
    assert!(inverted.permits(20 * 60), "evening is inside when inverted");
}

#[test]
fn disabled_permits_everything_and_invalid_permits_nothing() {
    for minutes in [0, 720, 1439] {
        assert!(GitOperationWindow::Disabled.permits(minutes));
        assert!(!GitOperationWindow::Invalid.permits(minutes));
    }
}

#[test]
fn every_listed_spelling_classifies_as_a_protected_operation() {
    let protected = [
        "git commit -m wip",
        "git push origin main",
        "git branch feature/x",
        "git branch --porcelain feature/x",
        "git branch -c old new",
        "git branch --copy old new",
        "git checkout -b feature/x",
        "git checkout -B feature/x",
        "git checkout --orphan feature/x",
        "git switch -c feature/x",
        "git switch -C feature/x",
        "git switch --create feature/x",
        "git switch --force-create feature/x",
        "git switch --orphan feature/x",
        "git worktree add ../wt-1 -b feature/x",
        "git worktree add -b feature/x ../wt-1",
        "git worktree add ../wt-1 -B feature/x",
        "git worktree add --orphan ../wt-1",
    ];
    for line in protected {
        let (action, _) = classify("Bash", &json!({"command": line}));
        assert!(
            window::action_is_protected(&action),
            "{line:?} classified as {action:?}, which the schedule would not stop"
        );
    }
}

#[test]
fn worktree_only_paths_and_reads_stay_unprotected() {
    let unprotected = [
        "git status",
        "git log --oneline",
        "git worktree add ../wt-1",
        "git worktree add ../wt-1 main",
        "git worktree list",
        "git checkout main",
        "git switch main",
        "git branch -d feature/x",
        "git branch --delete feature/x",
        "git branch -u origin/main",
        "git branch --set-upstream-to origin/main",
        "git branch --set-upstream-to=origin/main",
        "git branch --no-set-upstream-to origin/main",
        "git branch --list",
        "git branch --show-current",
    ];
    for line in unprotected {
        let (action, _) = classify("Bash", &json!({"command": line}));
        assert!(
            !window::action_is_protected(&action),
            "{line:?} classified as {action:?}, which the schedule would stop"
        );
    }
}

#[test]
fn every_write_to_github_classifies_as_a_protected_operation() {
    let protected = [
        "gh pr merge 7 --squash",
        "gh pr create --title x --body y",
        "gh pr ready 7",
        "gh pr edit 7 --add-label x",
        "gh release create v1",
        "gh release delete v1",
        "git merge feature/x",
        "git tag v1",
        "gh issue comment 5 --body hi",
        "gh issue edit 5 --add-label x",
        "gh issue develop 5 --name x",
        "gh label create wip",
        "gh api -X POST repos/o/r/issues/5/comments",
        "gh api --method=PATCH repos/o/r/issues/5",
        "gh api -X DELETE repos/o/r/issues/5",
        "gh api graphql -f query='mutation { x }'",
    ];
    for line in protected {
        let (action, _) = classify("Bash", &json!({"command": line}));
        assert!(
            window::action_is_protected(&action),
            "{line:?} classified as {action:?}, which the schedule would not stop"
        );
    }
}

#[test]
fn reads_from_github_stay_unprotected() {
    let unprotected = [
        "gh issue view 5",
        "gh issue list",
        "gh pr view 7",
        "gh pr list",
        "gh pr checks 7",
        "gh api repos/o/r/issues/5",
        "gh api -X GET repos/o/r/issues/5",
        "gh release list",
        "git tag -l",
        "git merge-base main HEAD",
    ];
    for line in unprotected {
        let (action, _) = classify("Bash", &json!({"command": line}));
        assert!(
            !window::action_is_protected(&action),
            "{line:?} classified as {action:?}, which the schedule would stop"
        );
    }
}

#[test]
fn a_remote_write_is_not_watched_when_nothing_closes_the_window() {
    for line in ["gh issue comment 5 --body hi", "gh pr ready 7"] {
        let (action, how) = classify("Bash", &json!({"command": line}));
        assert!(
            matches!(action, Action::Remote { .. }),
            "{line:?} read as {action:?}"
        );
        assert_eq!(how, estigia::harness::Sensitivity::Routine);
    }
}

#[test]
fn ordinary_file_writes_are_not_protected() {
    let (action, _) = classify("Edit", &json!({"file_path": "src/main.rs"}));
    assert!(
        matches!(action, Action::Write { .. }),
        "an edit stopped being a write: {action:?}"
    );
    assert!(
        !window::action_is_protected(&action),
        "an edit reads as schedule-protected: {action:?}"
    );
}

#[test]
fn the_local_clock_answers_in_minutes_since_monday() {
    let now = window::local_minutes_now().expect("this machine has a local clock");
    assert!(now < 7 * 1440, "a local minute is inside the week: {now}");
}

const MON: u16 = 0;
const TUE: u16 = 1;
const WED: u16 = 2;
const THU: u16 = 3;
const FRI: u16 = 4;
const SAT: u16 = 5;
const SUN: u16 = 6;

fn week(day: u16, hours: u16, minutes: u16) -> u16 {
    day * 1440 + hours * 60 + minutes
}

#[test]
fn weekday_forms_parse_and_render_canonically() {
    for (written, canonical) in [
        ("enabled mon-fri 08:00-17:00 inverted", None),
        ("enabled sat,sun 10:00-14:00", None),
        ("enabled mon-wed,fri 09:00-17:00", None),
        (
            "enabled MON-Fri 09:00-17:00",
            Some("enabled mon-fri 09:00-17:00"),
        ),
        ("enabled sun 09:00-17:00", None),
        (
            "enabled fri-mon 09:00-17:00",
            Some("enabled mon,fri-sun 09:00-17:00"),
        ),
        (
            "enabled fri,sat,mon 09:00-17:00",
            Some("enabled mon,fri,sat 09:00-17:00"),
        ),
        ("enabled mon-sun 09:00-17:00", None),
    ] {
        let parsed = enabled(written);
        assert_eq!(parsed.as_value(), canonical.unwrap_or(written));
        assert_eq!(enabled(&parsed.as_value()), parsed);
        let mut config = Config::default();
        Setting::GitOperationWindow
            .apply(&mut config, written)
            .unwrap_or_else(|refusal| panic!("{written:?} was refused: {refusal}"));
        assert_eq!(
            Setting::GitOperationWindow.value_of(&config),
            canonical.unwrap_or(written)
        );
    }
}

#[test]
fn invalid_day_sets_are_refused() {
    for bad in [
        "enabled mon,mon 09:00-17:00",
        "enabled mon-fri,wed 09:00-17:00",
        "enabled fri-mon,sun 09:00-17:00",
        "enabled mon, 09:00-17:00",
        "enabled ,mon 09:00-17:00",
        "enabled mon,,tue 09:00-17:00",
        "enabled mon- 09:00-17:00",
        "enabled -fri 09:00-17:00",
        "enabled funday 09:00-17:00",
        "enabled mon-fri-sun 09:00-17:00",
        "enabled mon-fri",
        "enabled mon-fri inverted",
        "enabled mon fri 09:00-17:00",
    ] {
        assert!(
            GitOperationWindow::parse(bad).is_none(),
            "{bad:?} parsed as a schedule"
        );
    }
}

#[test]
fn a_window_without_days_permits_every_day_as_before() {
    let window = enabled("enabled 09:00-17:00");
    for day in MON..=SUN {
        assert!(window.permits(week(day, 12, 0)));
        assert!(!window.permits(week(day, 20, 0)));
    }
}

#[test]
fn a_day_set_restricts_the_range_to_those_days() {
    let window = enabled("enabled mon-fri 08:00-17:00");
    for day in [MON, TUE, WED, THU, FRI] {
        assert!(window.permits(week(day, 8, 0)));
        assert!(!window.permits(week(day, 17, 0)));
    }
    assert!(!window.permits(week(SAT, 12, 0)));
    assert!(!window.permits(week(SUN, 12, 0)));

    let wrapping = enabled("enabled fri-mon 08:00-17:00");
    for day in [FRI, SAT, SUN, MON] {
        assert!(wrapping.permits(week(day, 12, 0)));
    }
    for day in [TUE, WED, THU] {
        assert!(!wrapping.permits(week(day, 12, 0)));
    }
}

#[test]
fn inversion_is_the_exact_complement_over_the_whole_week() {
    let normal = enabled("enabled mon-fri 08:00-17:00");
    let inverted = enabled("enabled mon-fri 08:00-17:00 inverted");
    for minute in (0..7 * 1440).step_by(13) {
        assert_ne!(normal.permits(minute), inverted.permits(minute), "{minute}");
    }
    assert!(inverted.permits(week(SAT, 12, 0)));
    assert!(inverted.permits(week(WED, 20, 0)));
    assert!(!inverted.permits(week(WED, 12, 0)));
}

#[test]
fn a_range_crossing_midnight_belongs_to_the_day_it_starts_on() {
    let window = enabled("enabled fri 22:00-06:00");
    assert!(window.permits(week(FRI, 22, 0)));
    assert!(window.permits(week(FRI, 23, 59)));
    assert!(window.permits(week(SAT, 0, 0)));
    assert!(window.permits(week(SAT, 5, 59)));
    assert!(!window.permits(week(SAT, 6, 0)));
    assert!(!window.permits(week(SAT, 22, 0)));
    assert!(!window.permits(week(FRI, 5, 0)));
    assert!(!window.permits(week(FRI, 21, 59)));

    let sunday = enabled("enabled sun 22:00-06:00");
    assert!(
        sunday.permits(week(MON, 5, 59)),
        "Sunday night runs into Monday"
    );
    assert!(!sunday.permits(week(MON, 22, 0)));
    assert!(!sunday.permits(week(SUN, 5, 0)));
}
