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

   Claude Code's `review-blind.md` lives outside the skill tree and uses that same path-only
   ownership record. Setup recursively parses the user agent tree before writing any setup artifact: a
   second file carrying the reserved YAML identity, an unprovable candidate, an unowned canonical file,
   or an owned canonical file whose bytes differ from **every rendering Estigia can produce** of the
   embedded definition is preserved and refused. Every rendering, not the embedded bytes themselves:
   `Evidence standard` decides that file's tool grant, so the embedded copy is a template and the raw
   template is exactly what a valid install never holds.
   Setup repeats that decision immediately before the reviewer step. A fresh path is recorded as owned
   before its bytes are written, then a complete staged body is linked into place without replacement;
   a file another actor created after either read is preserved, and ownership added by the losing create
   is removed. An interrupted create remains exactly retryable. Comparison follows established setup
   text normalization: CRLF versus LF and a missing final newline do not count as an edit; every other
   textual difference does. There is no generic outside-content digest or definition reconciler. Fresh
   and upgrade plans report the resulting `installed.json` ownership mutation exactly once as a skill
   create or update, matching the real action manifest.

   Two more definitions live outside the skill tree under that same record, and their rule is a
   third one rather than either of the two above. `implementer.md` and `analyst.md` are written only
   where `Delegated workers` names them — not `Planning`, and not `Model routing`, which says what a
   named worker runs on and never whether it exists. A definition already at one of those paths that
   Estigia did not create is **refused**, in the preflight and again immediately before the write,
   rather than overwritten the way an SDD phase definition is: `analyst` is a name another harness's
   orchestrator answers to, so replacing it would take somebody's own file with nothing recorded and
   nothing to give back. Taking a name out of that row retracts the file on the next run,
   which is what makes naming it consent rather than a one-way door. They are not alone in that: a
   plain `setup` also retracts an SDD phase definition once `Planning` stops running its phase, and
   drops a retired skill file. What is particular to these two is that a **configuration row** and
   not a protocol decides it.

   Uninstall deletes a textually unchanged owned reviewer before forgetting it or removing the skill ledger. A
   failed deletion therefore retains the evidence needed by an exact retry. Changed bytes are kept and
   their path is relinquished. This rule is deliberately specific to the reviewer; it does not broaden
   or repair the older SDD-definition ownership behavior, and the delegated workers'
   refuse-rather-than-replace is a third rule beside both. Configuration writes never touch the file.

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
   removed. Both are measured by `uninstall_leaves_the_checkouts_answers_and_the_list_of_checkouts`.

   `~/.estigia/` splits the difference, and the line between the two halves is *live run state*
   against *what the person keeps*. The run pointers, the ledger and any stand-down record who
   holds what and what was last asked: they go with the **last** agent, so `uninstall --all` with
   nothing left installed takes them — by name, and an empty directory only, so a file of the
   operator's own keeps the directory. Taking them at that point is the operator's recorded
   requirement and it is deliberate; taking them while any agent remains would silently strip a
   live run of the file its gate reads, which is why a plain reinstall never touches them and
   `setup` does not either. The machine-wide binary lifecycle evidence under `~/.estigia/lifecycle`
   belongs to the person rather than to any adapter and is not named by the removal, so it stays
   with whatever else is there. Both halves are measured by
   `taking_estigia_out_takes_its_own_state_with_it_and_not_before` and
   `a_live_run_pointer_survives_a_plain_reinstall`.

   A third leave is temporary: nine adapters share `~/.agents/skills`, so taking one of them out
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
