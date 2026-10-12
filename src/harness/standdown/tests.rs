use super::*;

const NOON: u64 = 1_800_000_000;

#[test]
fn a_stand_down_with_nothing_to_answer_for_is_refused() {
    // The reason is what separates this from a switch. Whitespace is not a
    // reason, and accepting it would let the whole design be bypassed by
    // pressing space.
    for empty in ["", "   ", "\t\n"] {
        assert_eq!(
            declare(empty, 30, NOON, "operator").unwrap_err(),
            Rejected::NoReason,
            "{empty:?} passed as a reason"
        );
    }
    assert!(declare("the tracker is down", 30, NOON, "operator").is_ok());
}

#[test]
fn the_window_is_bounded_at_both_ends() {
    // Zero covers nothing: a stand-down that is already over is a refusal
    // dressed as an allowance.
    assert_eq!(
        declare("why", 0, NOON, "operator").unwrap_err(),
        Rejected::NoTime
    );

    // Exactly the cap is allowed; one minute past it is not. A window an
    // operator can set and forget is a switch with extra steps.
    let cap = LONGEST / 60;
    assert!(
        declare("why", cap, NOON, "operator").is_ok(),
        "the cap itself"
    );
    assert_eq!(
        declare("why", cap + 1, NOON, "operator").unwrap_err(),
        Rejected::TooLong
    );

    // And nothing overflows into a window that never ends.
    assert_eq!(
        declare("why", u64::MAX, NOON, "operator").unwrap_err(),
        Rejected::TooLong
    );
}

#[test]
fn it_stops_applying_the_second_it_says_it_will() {
    let declared = declare("the tracker is down", 30, NOON, "operator").expect("declared");
    assert_eq!(declared.until, NOON + 1800);

    assert!(declared.covers(NOON), "it did not apply when declared");
    assert!(declared.covers(NOON + 1799), "it stopped a second early");
    // Exclusive at the far end: an inclusive bound leaves one second of
    // loosened gate after the declared window, which nobody would ever see.
    assert!(
        !declared.covers(NOON + 1800),
        "it still applied at the second it named"
    );
    assert!(!declared.covers(NOON + 5000));

    assert_eq!(declared.remaining(NOON), 1800);
    assert_eq!(declared.remaining(NOON + 1800), 0, "time ran backwards");
    assert_eq!(declared.remaining(NOON + 99_999), 0);
}

#[test]
fn an_expired_record_is_evidence_and_not_a_stand_down() {
    let declared = declare("why", 1, NOON, "operator").expect("declared");

    assert_eq!(in_force(Some(&declared), NOON), Some(&declared));
    // Expired: not in force, and — the point — still returned by nobody as
    // in force while remaining on disk. A stand-down that erased itself is one
    // nobody can answer for afterwards.
    assert_eq!(in_force(Some(&declared), NOON + 60), None);
    assert_eq!(in_force(None, NOON), None);
}

#[test]
fn a_record_survives_a_round_trip_through_the_file() {
    // It is read by a later process than the one that wrote it, so the shape
    // has to be stable on disk and not just in memory.
    let declared = declare("the tracker is down", 45, NOON, "asanabrial").expect("declared");
    let text = serde_json::to_string(&declared).expect("it serialises");
    let read: StandDown = serde_json::from_str(&text).expect("it parses back");
    assert_eq!(read, declared);
    // The reason is carried, not summarised away: it is the whole record.
    assert!(text.contains("the tracker is down"));
    assert!(text.contains("asanabrial"));
}

#[test]
fn every_rejection_says_something_different_and_actionable() {
    let all = [Rejected::NoReason, Rejected::NoTime, Rejected::TooLong];
    let codes: std::collections::BTreeSet<&str> = all.iter().map(|why| why.code()).collect();
    assert_eq!(codes.len(), all.len(), "two rejections share a code");
    for why in all {
        assert!(!why.action().is_empty(), "{why:?} says nothing to do");
    }
    // The cap is quoted from the constant rather than written out again, so a
    // change to the window cannot leave the refusal telling the old number.
    assert!(
        Rejected::TooLong
            .action()
            .contains(&(LONGEST / 60).to_string()),
        "the refusal names a window the code does not enforce"
    );
}

#[test]
fn the_record_sits_beside_the_run_pointers_and_not_among_them() {
    // This test is the reason the defect lasted. Its name says *not among
    // them*; its assertion said `file.parent() == Some(runs)`, which is among
    // them and nothing else. It passed, so the fix would have failed it, and a
    // reader taking the name at its word had no reason to look.
    //
    // What the name means, checked: the record is a sibling of `runs/`, not an
    // entry in it. `unreadable_holdings` walks that directory and parses every
    // `.json` as a `Run`, so a stand-down inside it is a run pointer that will
    // not open — and the push guard refuses every push on the machine when it
    // finds one. Declaring a stand-down turned the gate up.
    let runs = std::path::Path::new("/state/runs");
    let file = path(runs);
    assert_ne!(
        file.parent(),
        Some(runs),
        "the record is among the pointers"
    );
    assert_eq!(file.parent(), runs.parent(), "and not somewhere else again");

    // And the walk does not see it, wherever a previous build left it. Checked
    // against the reader rather than asserted about the path, because the
    // reader is what refuses the push.
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the runs directory");
    std::fs::write(
        legacy_path(&state),
        serde_json::to_string(&StandDown {
            reason: "rotating a credential".to_owned(),
            declared_at: 1,
            until: 2,
            declared_by: "somebody".to_owned(),
        })
        .expect("serialises"),
    )
    .expect("a stand-down where the old build put it");
    assert!(
        crate::harness::session::unreadable_holdings(&state).is_empty(),
        "a leftover stand-down still reads as a run pointer nobody can open"
    );
}

#[test]
fn an_allowance_under_a_stand_down_never_reads_like_an_ordinary_one() {
    use crate::harness::Decision;
    use crate::outcome::{Refusal, Resolution};

    let declared = declare("the tracker is down", 30, NOON, "asanabrial").expect("declared");
    let denial = || {
        Decision::Deny(Box::new(Refusal::not_started(
            "out-of-phase",
            "this step lands the work",
            Resolution::run("estigia status"),
        )))
    };

    // With none declared, and with an expired one, the refusal stands.
    assert!(over(denial(), None, Some(NOON)).denies());
    assert!(
        over(denial(), Some(&declared), Some(NOON + 1800)).denies(),
        "an expired stand-down still opened the gate"
    );

    // In force: allowed, and the allowance carries who, why, how long is left,
    // and what it overrode. All four, because a trace missing any one of them
    // cannot be answered for later.
    let Decision::Allow(why) = over(denial(), Some(&declared), Some(NOON + 60)) else {
        panic!("the stand-down did not apply");
    };
    assert!(why.contains("asanabrial"), "who declared it is missing");
    assert!(why.contains("the tracker is down"), "the reason is missing");
    assert!(
        why.contains("29 minute"),
        "how long is left is missing: {why}"
    );
    assert!(why.contains("out-of-phase"), "what it overrode is missing");

    // Outside stays outside: claiming authority the run never swore would be a
    // stand-down inventing a gate to stand down.
    assert!(matches!(
        over(
            Decision::Outside(crate::harness::Aside::NothingSworn),
            Some(&declared),
            Some(NOON)
        ),
        Decision::Outside(crate::harness::Aside::NothingSworn)
    ));
    // And an ordinary allowance is not relabelled, or every write would look
    // like it needed a stand-down.
    let Decision::Allow(plain) = over(
        Decision::Allow("holds #12".to_owned()),
        Some(&declared),
        Some(NOON),
    ) else {
        panic!("an allowance became something else");
    };
    assert_eq!(plain, "holds #12");
}

#[test]
fn a_clock_that_will_not_answer_does_not_stand_the_gate_down() {
    use crate::harness::Decision;
    use crate::outcome::{Refusal, Resolution};

    // The gate read the clock with `map_or(0, ..)`, so a machine that could not
    // answer what time it is looked like the epoch — and the epoch is inside
    // every window anybody ever declared. A stand-down from last March, thirty
    // minutes long and long expired, was in force again; a live one never ran
    // out. The gate opening because a clock broke is the one direction a
    // harness must not fail in, and two lines above that call the same file
    // already refuses to read an unreadable stand-down file as one in force.
    let declared = declare("the tracker is down", 30, NOON, "asanabrial").expect("declared");
    let denial = || {
        Decision::Deny(Box::new(Refusal::not_started(
            "out-of-phase",
            "this step lands the work",
            Resolution::run("estigia status"),
        )))
    };

    // Live at the moment it was declared, and still refused without a clock.
    assert!(
        !over(denial(), Some(&declared), Some(NOON + 60)).denies(),
        "the premise is wrong: this stand-down is not in force at all"
    );
    assert!(
        over(denial(), Some(&declared), None).denies(),
        "a clock nobody could read stood the gate down"
    );

    // And an expired one is not revived by it either, which is the same fault
    // arriving from the past rather than the present.
    assert!(over(denial(), Some(&declared), None).denies());

    // What is not a refusal is still untouched: this narrows the stand-down,
    // it does not turn a broken clock into a denial of its own.
    assert!(matches!(
        over(
            Decision::Outside(crate::harness::Aside::NothingSworn),
            Some(&declared),
            None
        ),
        Decision::Outside(crate::harness::Aside::NothingSworn)
    ));
}

#[test]
fn a_record_declaring_longer_than_the_cap_is_not_one_this_build_honours() {
    // `LONGEST` is the sentence this module opens with: a stand-down "is capped,
    // because a window long enough to forget about is a switch with extra
    // steps". The cap was enforced in `declare` — the **write** path — and
    // nowhere on the way back in. So the file on disk was the only authority on
    // how long the gate stays down, and three readers asked it one question:
    // `now < until`.
    //
    // Nothing exotic gets you there. A record hand-edited by the operator who
    // wanted "just a bit longer", one copied between machines, one written by a
    // build whose cap was different. Any of them is the switch this module says
    // it refuses to be, and every write it lets through goes unadjudicated.
    //
    // The rule belongs in one place because three call sites ask it: `in_force`,
    // `doctor`'s stand-down row, and `stand-down`'s own reader. Two of them
    // reached past `in_force` to `covers` directly — a rule held in three places
    // is a rule that will disagree with itself.
    let now = 1_000_000_u64;
    let honest = super::declare("something urgent", 30, now, "operator").expect("a real one");
    assert!(
        honest.covers(now + 60),
        "an ordinary stand-down stopped applying"
    );
    assert!(
        super::in_force(Some(&honest), now + 60).is_some(),
        "the reader disagreed with the record it was given"
    );

    // A day, where four hours is the cap. Still inside `until`, and that was the
    // only thing anybody asked.
    let stretched = super::StandDown {
        reason: "just a bit longer".to_owned(),
        declared_at: now,
        until: now + 24 * 60 * 60,
        declared_by: "operator".to_owned(),
    };
    assert!(
        !stretched.covers(now + 60),
        "a record declaring six times the cap stood the gate down"
    );
    assert!(
        super::in_force(Some(&stretched), now + 60).is_none(),
        "the gate honoured a window it would have refused to grant"
    );

    // Exactly the cap is the cap: a bound the readers disagree with the writer
    // about would refuse the longest stand-down `declare` will actually issue.
    let longest = super::declare("the longest allowed", super::LONGEST / 60, now, "operator")
        .expect("the cap itself is grantable");
    assert!(
        longest.covers(now + super::LONGEST - 1),
        "the longest stand-down this build grants is one it will not honour"
    );

    // And a record stamped ahead of its own end still expires: the cap is a
    // second question, not a replacement for the first.
    assert!(
        !honest.covers(now + 30 * 60),
        "an expired stand-down came back because the cap said it was reasonable"
    );
}

/// A stand-down that has not begun does not stand the gate down.
///
/// [`LONGEST`] bounds how **wide** a window may be and says nothing about
/// where it sits, so moving both stamps forward together walks past it: a
/// record declared a year from now, thirty minutes wide, is a legal window
/// and the gate stayed down for the whole year.
///
/// This crate has already paid for the shape once, on a session's renewal
/// window. Here the thing held open is the gate being **off**, which is the
/// direction nothing may fail in.
#[test]
fn a_stand_down_stamped_ahead_of_the_clock_does_not_cover_anything() {
    let now = 1_700_000_000;
    let record = |declared_at: u64, until: u64| StandDown {
        reason: "measuring".to_owned(),
        declared_at,
        until,
        declared_by: "somebody".to_owned(),
    };

    // The floor: an ordinary stand-down still covers, or every assertion
    // below is about a function that answers no to everything.
    assert!(
        record(now, now + 1800).covers(now),
        "an ordinary stand-down stopped covering"
    );
    // And it stops when it says it does.
    assert!(!record(now - 3600, now - 1800).covers(now));
    // The cap still holds, which is the guard this one sits beside.
    assert!(!record(now, now + 365 * 86_400).covers(now));

    // Declared for later, legally wide: the case that walked past the cap.
    let ahead = record(now + 365 * 86_400, now + 365 * 86_400 + 1800);
    assert!(
        ahead.until.saturating_sub(ahead.declared_at) <= LONGEST,
        "the fixture is refused by the cap, so it poses nothing new"
    );
    assert!(
        now < ahead.until,
        "the fixture has already expired, so it poses nothing new"
    );
    assert!(
        !ahead.covers(now),
        "a stand-down declared a year from now held the gate down for the year"
    );

    // One second ahead is still ahead, and the moment it was declared is
    // covered — a reader stricter than the writer would refuse the record
    // `declare` had just written.
    assert!(!record(now + 1, now + 1801).covers(now));
    assert!(record(now, now + 1800).covers(now));
}

/// The three things `covers` insists on, each measured.
///
/// Its own documentation records a hand measurement against `doctor` — four
/// records, one machine — and **none of the three conditions was held by a
/// test**. Two of them turned out to be: mutating the far end (`now < until`
/// into `<=`) goes red. The **cap on the width** did not: doubling `LONGEST`
/// in the reader left the whole suite green, so a record claiming a window
/// wider than this build ever issues would have been honoured, and what a
/// stand-down holds open is the gate being off.
///
/// A record like that needs no adversary: a clock that is ahead — a dead RTC, a
/// restored snapshot, an NTP jump — writes one by itself, and this crate has
/// already paid once for a future stamp holding a renewal window open forever.
#[test]
fn a_window_is_covered_only_while_it_is_open_and_no_wider_than_the_cap() {
    let now = 1_800_000_000;
    let window = |declared_at: u64, until: u64| super::StandDown {
        reason: "a fixture".to_owned(),
        declared_at,
        until,
        declared_by: "somebody".to_owned(),
    };

    // The floor: an ordinary window covers now.
    assert!(
        window(now - 60, now + 60).covers(now),
        "a live stand-down stopped covering the present"
    );

    // The far end is exclusive: the second it expires, it is over.
    assert!(
        !window(now - 60, now).covers(now),
        "an expired window covered"
    );
    // And it has to have started. A record dated a year ahead, thirty minutes
    // wide, is inside the cap and must not cover anything.
    assert!(
        !window(now + 31_536_000, now + 31_536_000 + 1_800).covers(now),
        "a window declared in the future covered the present"
    );

    // The cap, inclusive at exactly `LONGEST` because `declare` grants that
    // much and a reader stricter than the writer would refuse what this build
    // issues.
    assert!(
        window(now, now + super::LONGEST).covers(now),
        "the longest window this build issues was refused by its own reader"
    );
    assert!(
        !window(now, now + super::LONGEST + 1).covers(now),
        "a window wider than the cap was honoured; the gate can be held off too long"
    );
}
