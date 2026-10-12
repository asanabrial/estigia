//! A marked block inside somebody else's file.
//!
//! Invariant three of four, and it lives here once rather than in each module
//! that needs it. Estigia never rewrites a user's file: it replaces the span
//! between two markers and writes everything else back byte for byte. That is
//! also why there are no backups — a marked block plus an exact inverse leaves
//! nothing to restore.

/// A pair of markers, and the pairs it supersedes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    /// The opening marker.
    pub begin: &'static str,
    /// The closing marker.
    pub end: &'static str,
    /// Marker pairs an earlier version wrote, recognised so an upgrade
    /// *replaces* the old block instead of leaving the file with two of them.
    pub superseded: &'static [(&'static str, &'static str)],
}

/// A located block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedBlock {
    /// Byte offset of the opening marker.
    pub start: usize,
    /// Byte offset just past the closing marker.
    pub end: usize,
    /// Everything between the markers, trimmed.
    pub body: String,
}

impl Fence {
    /// Finds this fence's block, falling back to any pair it supersedes.
    pub fn find(&self, text: &str) -> Option<ManagedBlock> {
        std::iter::once((self.begin, self.end))
            .chain(self.superseded.iter().copied())
            .find_map(|(begin, end)| locate(text, begin, end))
    }

    /// Whether the file already carries this fence.
    pub fn is_present(&self, text: &str) -> bool {
        self.find(text).is_some()
    }

    /// Writes `body` between the markers, creating the block at the end when
    /// it is absent. Everything outside the markers is preserved exactly.
    pub fn upsert(&self, existing: &str, body: &str) -> String {
        let text = existing.replace("\r\n", "\n");
        let block = format!("{}\n\n{}\n\n{}", self.begin, body.trim(), self.end);
        match self.find(&text) {
            Some(found) => {
                let spliced = format!("{}{block}{}", &text[..found.start], &text[found.end..]);
                let kept = found.start + block.len();
                self.without_superseded(&self.without_later_copies(spliced, kept))
            }
            None => {
                let head = text.trim_end();
                if head.is_empty() {
                    format!("{block}\n")
                } else {
                    format!("{head}\n\n{block}\n")
                }
            }
        }
    }

    /// Lifts the block out, leaving everything else as it was. Comes back
    /// empty when the block was all the file held; whether the file itself is
    /// deleted is the caller's question, and the gap is in the README's
    /// honesty section.
    pub fn remove(&self, existing: &str) -> String {
        let mut text = existing.replace("\r\n", "\n");
        if self.find(&text).is_none() {
            // Exactly what was read, not the normalised copy: handing back
            // `text` rewrote every line ending in a file this call took
            // nothing out of.
            return existing.to_owned();
        }
        while let Some(found) = self.find(&text) {
            let joined = format!("{}{}", text[..found.start].trim_end(), &text[found.end..]);
            let trimmed = joined.trim();
            text = if trimmed.is_empty() {
                String::new()
            } else {
                format!("{trimmed}\n")
            };
        }
        text
    }

    fn without_later_copies(&self, mut text: String, kept: usize) -> String {
        while let Some(found) = locate(&text[kept..], self.begin, self.end) {
            let head = text[..kept + found.start].trim_end();
            let tail = text[kept + found.end..].trim_start();
            text = if tail.is_empty() {
                format!("{head}\n")
            } else {
                format!("{head}\n\n{tail}")
            };
        }
        text
    }

    /// Drops blocks left under a name this fence supersedes. Returns the text
    /// unchanged when there is nothing stale.
    fn without_superseded(&self, text: &str) -> String {
        let mut left = text.to_owned();
        for &(begin, end) in self.superseded {
            while let Some(found) = locate(&left, begin, end) {
                let head = left[..found.start].trim_end();
                let tail = left[found.end..].trim_start();
                left = match (head.is_empty(), tail.is_empty()) {
                    (true, true) => String::new(),
                    (true, false) => tail.to_owned(),
                    (false, true) => format!("{head}\n"),
                    (false, false) => format!("{head}\n\n{tail}"),
                };
            }
        }
        left
    }
}

fn locate(text: &str, begin: &str, end: &str) -> Option<ManagedBlock> {
    let first = text.find(begin)?;
    let relative = text[first..].find(end)?;
    let stop = first + relative + end.len();
    // The **last** opening before that close, not the first: a truncated write
    // or a hand edit can leave an opening with nothing closing it, and taking
    // the first made the span swallow everything between the two markers —
    // where the operator's own text is. The orphaned opening is left where it
    // is: with no close there is no boundary to remove past without guessing.
    let start = text[..first + relative].rfind(begin).unwrap_or(first);
    let body = text[start + begin.len()..first + relative]
        .trim()
        .to_owned();
    Some(ManagedBlock {
        start,
        end: stop,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FENCE: Fence = Fence {
        begin: "<!-- BEGIN -->",
        end: "<!-- END -->",
        superseded: &[("<!-- OLD-BEGIN -->", "<!-- OLD-END -->")],
    };

    /// An opening marker with nothing closing it does not extend the block:
    /// taking the first opening swallowed everything between the two markers,
    /// including the operator's own text.
    #[test]
    fn an_opening_nothing_closed_does_not_swallow_what_follows_it() {
        let damaged = concat!(
            "mine, before

",
            "<!-- BEGIN -->
stale text nobody closed
",
            "MINE, AFTER THE DAMAGE

",
            "<!-- BEGIN -->
the block this build wrote
<!-- END -->
",
        );

        // The floor: the well-formed block really is found, or "it does not take
        // too much" is a sentence about a function that found nothing.
        let found = locate(damaged, FENCE.begin, FENCE.end).expect("the closed block is there");
        assert_eq!(
            found.body, "the block this build wrote",
            "the block read is not the one that is closed"
        );

        let left = FENCE.remove(damaged);
        assert!(
            left.contains("MINE, AFTER THE DAMAGE"),
            "removal took the operator's own paragraph with it:
{left}"
        );
        assert!(
            left.contains("mine, before"),
            "removal took the text before the damage:
{left}"
        );
        assert!(
            !left.contains("the block this build wrote"),
            "the block this build wrote is still there:
{left}"
        );

        let ordinary = "before

<!-- BEGIN -->
ours
<!-- END -->

after
";
        let left = FENCE.remove(ordinary);
        assert!(
            left.contains("before") && left.contains("after") && !left.contains("ours"),
            "an ordinary managed block stopped being removed whole:
{left}"
        );
    }

    #[test]
    fn upserting_replaces_the_block_and_keeps_everything_else() {
        let original = "# Notes\n\nkeep me\n\n<!-- BEGIN -->\n\nold\n\n<!-- END -->\n\nand me\n";
        let updated = FENCE.upsert(original, "new");
        assert!(updated.contains("keep me"));
        assert!(updated.contains("and me"));
        assert!(updated.contains("new"));
        assert!(!updated.contains("old"));
        assert_eq!(updated.matches(FENCE.begin).count(), 1);
    }

    #[test]
    fn a_file_carrying_both_names_keeps_one_block_and_it_is_the_written_one() {
        // With both names present the write used to update the current block
        // and leave the stale one above it, where the agent reaches it first.
        let both = "# Skill\n\n<!-- OLD-BEGIN -->\n\nstale\n\n<!-- OLD-END -->\n\n\
                    <!-- BEGIN -->\n\ncurrent\n\n<!-- END -->\n\ntrailing\n";
        let updated = FENCE.upsert(both, "written");

        assert_eq!(
            updated.matches("<!-- OLD-BEGIN -->").count(),
            0,
            "{updated}"
        );
        assert_eq!(updated.matches(FENCE.begin).count(), 1, "{updated}");
        assert!(!updated.contains("stale"), "{updated}");
        assert!(updated.contains("written") && updated.contains("trailing"));
        assert!(updated.starts_with("# Skill\n"), "{updated}");
        assert_eq!(FENCE.find(&updated).expect("a block").body, "written");
    }

    #[test]
    fn a_superseded_pair_is_replaced_rather_than_left_beside_the_new_one() {
        let original = "# Notes\n\n<!-- OLD-BEGIN -->\n\nold\n\n<!-- OLD-END -->\n";
        let updated = FENCE.upsert(original, "new");
        assert!(!updated.contains("OLD-BEGIN"));
        assert!(!updated.contains("old"));
        assert_eq!(updated.matches(FENCE.begin).count(), 1);
    }

    #[test]
    fn removing_is_the_exact_inverse_of_upserting() {
        let original = "# Notes\n\nkeep me\n";
        assert_eq!(FENCE.remove(&FENCE.upsert(original, "a block")), original);
    }

    #[test]
    fn a_file_that_held_only_the_block_comes_out_empty_but_is_not_deleted() {
        assert_eq!(FENCE.remove(&FENCE.upsert("", "a block")), "");
    }

    #[test]
    fn removing_a_fence_that_is_not_there_changes_nothing() {
        // Both line endings: `remove` used to hand the CRLF-normalised text
        // back, rewriting every line ending in a file it changed nothing in.
        for theirs in [
            "# Someone else's file\n\n<!-- BEGIN OTHER TOOL -->\ntheirs\n<!-- END OTHER TOOL -->\n",
            "# Someone else's file\r\n\r\n<!-- BEGIN OTHER TOOL -->\r\ntheirs\r\n<!-- END OTHER TOOL -->\r\n",
        ] {
            assert_eq!(FENCE.remove(theirs), theirs);
        }
    }

    #[test]
    fn crlf_does_not_produce_a_second_block() {
        let original = "# Notes\r\n\r\n<!-- BEGIN -->\r\n\r\nold\r\n\r\n<!-- END -->\r\n";
        let updated = FENCE.upsert(original, "new");
        assert_eq!(updated.matches(FENCE.begin).count(), 1);
        assert!(!updated.contains("old"));
    }

    #[test]
    fn an_opening_marker_with_no_closing_one_is_not_a_block() {
        // Half a block is a file somebody edited by hand. Treating it as found
        // would splice the replacement over the rest of their document.
        let truncated = "# Notes\n\n<!-- BEGIN -->\n\nunterminated\n";
        assert!(FENCE.find(truncated).is_none());
        let updated = FENCE.upsert(truncated, "new");
        assert!(
            updated.contains("unterminated"),
            "their text was overwritten"
        );
    }

    fn doubled() -> String {
        "# mine\n\n<!-- BEGIN -->\n\nold\n\n<!-- END -->\n\nmiddle\n\n<!-- BEGIN -->\n\nstale\n\n<!-- END -->\n\nafter\n"
            .to_owned()
    }

    #[test]
    fn removing_takes_every_copy_of_the_block() {
        let left = FENCE.remove(&doubled());
        assert!(!FENCE.is_present(&left), "{left}");
        assert_eq!(left, "# mine\n\nmiddle\n\nafter\n");
    }

    #[test]
    fn upserting_leaves_exactly_one_block_holding_the_new_body() {
        let updated = FENCE.upsert(&doubled(), "new");
        assert_eq!(updated.matches(FENCE.begin).count(), 1, "{updated}");
        assert!(
            !updated.contains("stale") && !updated.contains("old"),
            "{updated}"
        );
        assert_eq!(FENCE.find(&updated).expect("a block").body, "new");
        assert!(
            updated.starts_with("# mine\n\n")
                && updated.contains("middle")
                && updated.ends_with("after\n"),
            "{updated}"
        );
    }

    #[test]
    fn is_present_agrees_with_find() {
        let with = FENCE.upsert("", "body");
        assert!(FENCE.is_present(&with));
        assert!(!FENCE.is_present("nothing here"));
    }
}
