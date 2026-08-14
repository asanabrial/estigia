//! A marked block inside somebody else's file.
//!
//! Invariant three of four, and it lives here once rather than in each module
//! that needs it. Estigia never rewrites a user's file: it replaces the span
//! between two markers and writes everything else back byte for byte. That is
//! also why there are no backups — a marked block plus an exact inverse leaves
//! nothing to restore.
//!
//! There are two of these: the configuration table inside the installed
//! `SKILL.md`, and the workflow directive inside an agent's always-loaded
//! instruction file. They were written twice, forty lines apart, before it was
//! obvious they were the same forty lines. Two copies of the invariant is one
//! copy too many for something whose whole job is not to damage a file it does
//! not own.

/// A pair of markers, and the pairs it supersedes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    /// The opening marker.
    pub begin: &'static str,
    /// The closing marker.
    pub end: &'static str,
    /// Marker pairs an earlier version wrote.
    ///
    /// Recognised so an upgrade *replaces* the old block instead of leaving the
    /// file with two of them — which is what a rename costs when nobody
    /// remembers the old name. That holds however many names are present: a
    /// write leaves exactly one block, because an agent reading a file with two
    /// of them reads whichever it reaches first, which is the stale one as often
    /// as not.
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
    ///
    /// A read-only question, and a cheap one: callers ask it to report what is
    /// already configured, not to decide whether a write is safe.
    pub fn is_present(&self, text: &str) -> bool {
        self.find(text).is_some()
    }

    /// Writes `body` between the markers, creating the block at the end when
    /// it is absent.
    ///
    /// Everything outside the markers is preserved exactly, including a file
    /// that is nothing but somebody's own notes.
    pub fn upsert(&self, existing: &str, body: &str) -> String {
        let text = existing.replace("\r\n", "\n");
        let block = format!("{}\n\n{}\n\n{}", self.begin, body.trim(), self.end);
        match self.find(&text) {
            Some(found) => {
                let spliced = format!("{}{block}{}", &text[..found.start], &text[found.end..]);
                self.without_superseded(&spliced)
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

    /// Lifts the block out, leaving everything else as it was.
    ///
    /// Comes back empty when the block was all the file held, and says nothing
    /// about whether the file survives — that is the caller's, and `setup`'s
    /// answer is to delete it. Which is right for a file Estigia created and
    /// wrong for one an operator kept empty, and the two cannot be told apart
    /// from here or from there; the README's honesty section carries the gap
    /// and what closing it needs.
    ///
    /// This used to claim the file itself stays, on the reasoning that an agent
    /// finding its instruction file missing behaves differently from one
    /// finding it empty. The reasoning holds. The claim did not: nothing in
    /// this function decides it, and the caller had never done what it said.
    pub fn remove(&self, existing: &str) -> String {
        let text = existing.replace("\r\n", "\n");
        let Some(found) = self.find(&text) else {
            // Exactly what was read, not the normalised copy this looked in.
            // Handing back `text` rewrote every line ending in a file the call
            // took nothing out of — invariant two, arrived at from a third
            // direction after the JSON and TOML strippers.
            return existing.to_owned();
        };
        let joined = format!("{}{}", text[..found.start].trim_end(), &text[found.end..]);
        let trimmed = joined.trim();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("{trimmed}\n")
        }
    }

    /// Drops blocks left under a name this fence supersedes.
    ///
    /// `find` reads the old pair only as a *fallback*, so a file carrying both
    /// names kept the stale one: the write updated the current block and walked
    /// past the other. Which is the file with two configuration tables that
    /// `superseded` exists to prevent — and the agent reads the one it reaches
    /// first, not the one that was just written.
    ///
    /// Returns the text unchanged when there is nothing stale, so the ordinary
    /// write stays the byte-for-byte splice it has always been.
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
    // The **last** opening before that close, not the first. A file can hold an
    // opening marker with nothing closing it — a truncated write, a crash
    // mid-rename, an operator who cut a paragraph — and `setup` then finds no
    // block and appends a whole one, leaving `BEGIN … BEGIN … END`. Taking the
    // first opening makes that one span, so the removal swallowed **everything
    // between the two markers**, which is where the operator's own text is.
    //
    // Measured, one `CLAUDE.md`, three commands:
    //
    // ```text
    // setup                       -> 1 BEGIN, 1 END
    // (the END marker is lost)    -> 1 BEGIN, 0 END, and a paragraph after it
    // setup                       -> 2 BEGIN, 1 END
    // uninstall                   -> the paragraph is gone
    // ```
    //
    // Which is the one thing this crate promises about somebody else's file.
    //
    // The orphaned opening is left where it is, and deliberately: there is no
    // close to say how far Estigia's text ran, so removing anything after it
    // would be guessing at the boundary that has just been lost — the same
    // guess that ate the paragraph.
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

    /// An opening marker with nothing closing it does not extend the block.
    ///
    /// A file can hold one: a truncated write, a crash mid-rename, an operator
    /// who cut a paragraph out of the middle. `upsert` then finds no block and
    /// appends a whole one, so the file carries `BEGIN … BEGIN … END` — and
    /// taking the **first** opening made that one span. Removing it swallowed
    /// everything between the two markers, which is where the operator's own
    /// text is.
    ///
    /// Measured through the binary before this, on one `CLAUDE.md`: `setup`,
    /// the closing marker lost, `setup` again, `uninstall` — and a paragraph
    /// the operator had written after the damage was gone.
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

        // And an ordinary file is untouched by the change: one opening, one
        // close, and the whole block goes.
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
        // `find` reads the old pair as a fallback, so with both names present it
        // returned the current block, the write updated that, and the stale one
        // stayed — above it, where the agent reaches it first. Two configuration
        // tables in one file, disagreeing, which is the exact thing `superseded`
        // is for. `setup` never showed it because it rewrites the whole file;
        // `config set` splices, and that is the command an operator runs daily.
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
        // What was never ours is still here, and still readable.
        assert!(updated.starts_with("# Skill\n"), "{updated}");

        // Reading agrees with writing: one block, and it says what was written.
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
        // Both line endings. The name of this test is the whole claim, and it
        // was only ever checked against the one spelling that made it true:
        // `remove` normalised CRLF on the way in and handed the normalised text
        // back, so a file of somebody's on Windows came out of a call that
        // removed nothing with every line ending rewritten.
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

    #[test]
    fn is_present_agrees_with_find() {
        let with = FENCE.upsert("", "body");
        assert!(FENCE.is_present(&with));
        assert!(!FENCE.is_present("nothing here"));
    }
}
