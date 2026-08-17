# What it writes, and how it takes it back

Estigia goes into files somebody else owns. This is the account of what it puts there, what it
removes when it leaves, and the two things it deliberately refuses to remove.


Five invariants, and the tests are named after them:

1. **`is_configured` is a query**, separate from `setup`. "Where did I install this?" is the
   question people actually arrive with.
2. **Install says when it is writing over somebody else's file.** Estigia ships upstream's skill
   under upstream's name, so an operator who already runs `issue-flow` has files at those paths.
   Install writes its own copy over them and reports each as `REPLACE` rather than `update`, and
   `--dry-run` says so before anything happens. Nothing keeps the old contents: uninstall leaves
   the file, because it is not Estigia's to remove, so what stands afterwards is Estigia's copy.
3. **Uninstall is the exact inverse.** It removes what Estigia **created** and nothing else — not
   another tool's block, not your own notes, not a file you dropped beside the references, and not a
   file that was already there when Estigia wrote over it. That last one is the common case, not the
   corner: Estigia installs upstream's skill under upstream's name, so a directory that already holds
   `issue-flow` is the *same* directory. Install therefore records what it created, in
   `<skill>/.estigia/installed.json`, because nothing on disk can tell the two apart afterwards —
   an overwritten file and a created one both hold Estigia's text. Delete that record and uninstall
   removes nothing, which is the honest reading of no evidence.

   Claude Code's static `review-blind.md` lives outside the skill tree and uses that same path-only
   ownership record. Setup preflights this one reserved name before writing any setup artifact: an
   unowned existing file or an owned file whose bytes differ from the embedded definition is preserved
   and refused. A fresh path is recorded as owned before its bytes are written, so an interrupted create
   remains exactly retryable. Comparison follows established setup text normalization: CRLF versus LF
   and a missing final newline do not count as an edit; every other textual difference does. There is
   no generic outside-content digest or definition reconciler. Fresh and upgrade plans report the
   resulting `installed.json` ownership mutation exactly once as a skill create or update, matching
   the real action manifest.

   Uninstall deletes a textually unchanged owned reviewer before forgetting it or removing the skill ledger. A
   failed deletion therefore retains the evidence needed by an exact retry. Changed bytes are kept and
   their path is relinquished. This rule is deliberately specific to the reviewer; it does not broaden
   or repair the older SDD-definition ownership behavior. Configuration writes never touch the file.

   **By name, and by content on the way in — not yet on the way out.** The record used to store the
   paths an install created and nothing about what it wrote there, so *did you change this, or did an
   older build write it* had no answer: a file Estigia created and you later edited was rewritten by
   `sync` on a plain `update` line, the same word a version bump gets. It now stores a **digest of
   what it last wrote** at each path, which answers exactly that question and nothing else. An
   install that finds one of its own files changed since says `OVERWRITE`, and a machine merely one
   release behind still reads as `update`, because its record predates digests and holds none.

   **`OVERWRITE` and `REPLACE` are two words because they are two things**, and they were one for
   several rounds. `REPLACE` is a file that was never Estigia's; `OVERWRITE` is Estigia's file with
   your edit in it. Sharing the word meant sharing the sentence under it, and `sync` announced *"1
   file(s) were already here and are not Estigia's"* about a file `estigia setup` had installed
   thirty seconds earlier — which sends somebody whose own edit has just been discarded looking for
   a file that was never there. What you do next differs: one is a stranger's file you may not have
   known about, the other is work you have lost.

   **And on the way out.** `uninstall` used to take a file you had edited with the rest, named on its
   own `remove` line like every other and in no way marked as one you had touched — measured: a
   `bindings/github.md` rewritten whole and an `rdd.md` with a note appended were both taken, each
   printed among the eighteen. The same digest answers it: a file whose contents have moved since
   Estigia wrote them is **kept**, and named on its own line so nothing is left silently. An
   untouched one still goes, and so does every file on a machine whose record predates digests —
   otherwise the fix would leave whole installations behind, which is the failure it exists to
   prevent, inverted.

   Two things Estigia created it deliberately leaves, and they are named here because "removes what
   Estigia created" would otherwise read as *all* of it. A checkout's own rows in
   `.git/estigia/` stay: uninstall takes an **agent** out and was never given a repository, so
   reaching into whichever checkout somebody happened to be standing in is the failure this design
   refuses — `config forget` is the one command that removes them, and it says which file it
   removed. `~/.estigia/` stays for the same reason in the other direction: the screen's language,
   the list of checkouts and machine-wide binary lifecycle evidence belong to the person rather than
   to any adapter, and uninstalling one agent out of eleven must not take them. Both are measured by
   `uninstall_leaves_the_checkouts_answers_and_the_list_of_checkouts`.

   A third leave is temporary: eight adapters share `~/.agents/skills`, so taking one of them out
   leaves the skill for the agents that remain and it goes with the last of them. The note that
   accounts for the surviving directory names **only the files no line of the run already named** —
   not everything still on disk. Naming everything was measured, and it said sixteen files were the
   operator's where one was, listing `SKILL.md`, the transport and Estigia's own record among them:
   the sentence that exists to answer *it did not touch my things* both buried its real answer and
   invited a person to delete a skill two configured agents were reading.
   `the_skill_left_for_another_agent_is_not_reported_as_the_operator_s` holds both halves, the second
   being that a file reported `unknown` — no record, so nothing shown to be Estigia's — was reported
   as the operator's in the same output.
4. **Everything Estigia writes into a shared file lives between markers**, and a file that held
   nothing but our block does not survive as an empty husk. There are no backups because there is
   nothing to restore. Markdown comes back byte for byte, and so does JSON: the indentation a file
   was written with is read off it and used again, so a settings file Estigia went into to add one
   hook is not handed back reindented.
5. **`--dry-run` reports what the real run does**, checked by a test that runs both and compares.
