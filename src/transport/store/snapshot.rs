use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::{Failure, Store, unreadable};

/// How old the newest snapshot must be before another is taken: one day.
pub(super) const SNAPSHOT_INTERVAL: u64 = 24 * 60 * 60;

/// How many snapshots are kept before the oldest are removed.
pub(super) const SNAPSHOT_KEEP: usize = 3;

/// The directory snapshots live in, beside the store file.
pub(crate) fn snapshots_dir(store_path: &Path) -> PathBuf {
    store_path.with_file_name("snapshots")
}

/// Whether a snapshot is due, given the age of the newest one in seconds.
///
/// Pure and fed, so the cadence can be tested without waiting a day.
pub(super) fn snapshot_due(newest_age: Option<u64>) -> bool {
    newest_age.is_none_or(|age| age >= SNAPSHOT_INTERVAL)
}

/// The age of the newest snapshot in seconds, `None` when there is none.
pub(crate) fn snapshot_age(store_path: &Path, now: u64) -> Option<u64> {
    stamps(store_path)
        .last()
        .map(|stamp| now.saturating_sub(*stamp))
}

/// The last snapshot's age in words, for the report a person reads.
pub(crate) fn age_in_words(age: u64) -> String {
    match age {
        0..=59 => format!("{age}s"),
        60..=3599 => format!("{}m", age / 60),
        3600..=86_399 => format!("{}h", age / 3600),
        _ => format!("{}d", age / 86_400),
    }
}

/// Writes a snapshot when one is due and prunes the ones beyond retention.
///
/// Answers the failure rather than swallowing it: the caller records it, and a
/// snapshot is never allowed to fail the operation that triggered it.
pub(super) fn take_snapshot(
    connection: &Connection,
    store_path: &Path,
    now: u64,
) -> Result<(), Failure> {
    if !snapshot_due(snapshot_age(store_path, now)) {
        return Ok(());
    }
    let directory = snapshots_dir(store_path);
    std::fs::create_dir_all(&directory).map_err(|error| {
        Failure::Read(format!("could not create {}: {error}", directory.display()))
    })?;
    let destination = directory.join(now.to_string());
    if !destination.exists() {
        let destination = destination.to_string_lossy().into_owned();
        connection
            .execute("VACUUM INTO ?1", [destination.as_str()])
            .map_err(unreadable("snapshot itself"))?;
    }
    prune(store_path);
    Ok(())
}

fn stamps(store_path: &Path) -> Vec<u64> {
    let Ok(entries) = std::fs::read_dir(snapshots_dir(store_path)) else {
        return Vec::new();
    };
    let mut stamps: Vec<u64> = entries
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse().ok())
        })
        .collect();
    stamps.sort_unstable();
    stamps
}

fn prune(store_path: &Path) {
    for stamp in stamps(store_path).into_iter().rev().skip(SNAPSHOT_KEEP) {
        let _ = std::fs::remove_file(snapshots_dir(store_path).join(stamp.to_string()));
    }
}

impl Store {
    /// Snapshots the store when one is due, and records the failure when one is
    /// not written, so the operation that triggered it goes on.
    pub(super) fn snapshot(&self, store_path: &Path, now: u64) {
        if take_snapshot(self.connection(), store_path, now).is_err() {
            let _ = self.note_snapshot_failure();
        }
    }
}
