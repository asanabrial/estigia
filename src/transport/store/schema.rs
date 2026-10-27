pub(super) const MIGRATIONS: &[&str] = &[
    "
    CREATE TABLE repos (
        id INTEGER PRIMARY KEY,
        host TEXT NOT NULL,
        owner TEXT NOT NULL,
        name TEXT NOT NULL,
        UNIQUE (host, owner, name)
    );
    CREATE TABLE issues (
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        number INTEGER NOT NULL,
        state TEXT CHECK (state IN
            ('analysis', 'ready', 'in-progress', 'review', 'blocked', 'done')),
        identity TEXT,
        history TEXT NOT NULL CHECK (history IN ('native', 'imported')),
        imported_at TEXT,
        PRIMARY KEY (repo_id, number)
    );
    CREATE TABLE entries (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        repo_id INTEGER NOT NULL,
        issue INTEGER NOT NULL,
        created_at TEXT NOT NULL,
        body TEXT NOT NULL,
        kind TEXT,
        run_id TEXT,
        op_id TEXT,
        viewer_did_author INTEGER NOT NULL,
        includes_created_edit INTEGER NOT NULL,
        source TEXT NOT NULL CHECK (source IN ('local', 'import')),
        source_comment_id TEXT,
        FOREIGN KEY (repo_id, issue) REFERENCES issues(repo_id, number)
    );
    CREATE UNIQUE INDEX entry_op ON entries (repo_id, issue, op_id)
        WHERE op_id IS NOT NULL;
    CREATE UNIQUE INDEX entry_import ON entries (repo_id, source_comment_id)
        WHERE source_comment_id IS NOT NULL;
    CREATE TRIGGER entries_no_update BEFORE UPDATE ON entries
        BEGIN SELECT RAISE(ABORT, 'append-only'); END;
    CREATE TRIGGER entries_no_delete BEFORE DELETE ON entries
        BEGIN SELECT RAISE(ABORT, 'append-only'); END;
    CREATE TABLE calls (
        id INTEGER PRIMARY KEY,
        operation TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('graphql', 'rest', 'other')),
        count INTEGER NOT NULL,
        at INTEGER NOT NULL
    );
    ",
    "
    ALTER TABLE issues ADD COLUMN synced_through TEXT;
    CREATE TABLE seen_comments (
        repo_id INTEGER NOT NULL,
        node_id TEXT NOT NULL,
        matched_seq INTEGER,
        PRIMARY KEY (repo_id, node_id)
    );
    ",
    "
    ALTER TABLE seen_comments ADD COLUMN github_created_at TEXT;
    ",
    "
    ALTER TABLE entries ADD COLUMN github_id INTEGER;
    ALTER TABLE seen_comments ADD COLUMN github_id INTEGER;
    ",
    "
    CREATE TABLE review_jobs (
        id TEXT PRIMARY KEY,
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        issue INTEGER NOT NULL,
        run_id TEXT NOT NULL,
        name TEXT NOT NULL,
        epoch TEXT NOT NULL,
        pr INTEGER NOT NULL,
        head TEXT NOT NULL,
        base TEXT NOT NULL,
        digest TEXT NOT NULL,
        payload_digest TEXT NOT NULL,
        offered TEXT NOT NULL,
        attempt TEXT,
        pid INTEGER,
        started_at INTEGER NOT NULL,
        deadline_at INTEGER NOT NULL,
        dir TEXT NOT NULL,
        state TEXT NOT NULL CHECK (state IN
            ('running', 'admitting', 'admitted', 'stale', 'abandoned'))
    );
    CREATE UNIQUE INDEX review_job_live ON review_jobs (repo_id, issue, epoch, name)
        WHERE state IN ('running', 'admitting');
    ",
    "
    CREATE TABLE blocked_clauses (
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        issue INTEGER NOT NULL,
        body TEXT NOT NULL,
        at TEXT NOT NULL
    );
    ",
    "
    CREATE TABLE decisions (
        at INTEGER,
        body TEXT NOT NULL
    );
    CREATE INDEX decisions_at ON decisions (at);
    CREATE TABLE roles (
        at INTEGER,
        body TEXT NOT NULL
    );
    CREATE INDEX roles_at ON roles (at);
    CREATE TABLE record_failures (
        kind TEXT PRIMARY KEY,
        count INTEGER NOT NULL
    );
    CREATE TABLE store_meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    ",
    "
    CREATE TABLE tasks (
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        issue INTEGER NOT NULL,
        id TEXT NOT NULL,
        text TEXT NOT NULL,
        route TEXT NOT NULL CHECK (route IN ('inline', 'delegated')),
        done INTEGER NOT NULL CHECK (done IN (0, 1)),
        commit_id TEXT,
        at TEXT NOT NULL,
        PRIMARY KEY (repo_id, issue, id)
    );
    ",
    "
    CREATE TABLE outbox (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        issue INTEGER NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('create', 'comment', 'reconverge', 'link')),
        payload TEXT NOT NULL,
        op_id TEXT NOT NULL UNIQUE,
        state TEXT NOT NULL CHECK (state IN ('queued', 'sending', 'sent')),
        enqueued_at TEXT NOT NULL,
        attempted_at TEXT,
        detail TEXT
    );
    CREATE UNIQUE INDEX outbox_one_reconverge ON outbox (repo_id, issue)
        WHERE kind = 'reconverge' AND state = 'queued';
    CREATE TABLE provisional_issues (
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        provisional INTEGER NOT NULL,
        real INTEGER,
        PRIMARY KEY (repo_id, provisional)
    );
    DROP TRIGGER entries_no_update;
    CREATE TRIGGER entries_no_update BEFORE UPDATE ON entries
        WHEN NOT (
            NEW.issue <> OLD.issue
            AND EXISTS (SELECT 1 FROM provisional_issues
                        WHERE repo_id = OLD.repo_id AND provisional = OLD.issue
                          AND real = NEW.issue)
            AND NEW.seq IS OLD.seq AND NEW.repo_id IS OLD.repo_id
            AND NEW.created_at IS OLD.created_at AND NEW.body IS OLD.body
            AND NEW.kind IS OLD.kind AND NEW.run_id IS OLD.run_id
            AND NEW.op_id IS OLD.op_id
            AND NEW.viewer_did_author IS OLD.viewer_did_author
            AND NEW.includes_created_edit IS OLD.includes_created_edit
            AND NEW.source IS OLD.source
            AND NEW.source_comment_id IS OLD.source_comment_id
            AND NEW.github_id IS OLD.github_id)
        BEGIN SELECT RAISE(ABORT, 'append-only'); END;
    ",
    "
    CREATE TABLE sessions (
        run_id TEXT PRIMARY KEY,
        runtime TEXT NOT NULL,
        session_id TEXT NOT NULL,
        repo_dir TEXT,
        started_at INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        ended_at INTEGER,
        end_reason TEXT
    );
    ",
    "
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        sender_run_id TEXT,
        recipient_run_id TEXT NOT NULL,
        body TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        delivered_at INTEGER
    );
    CREATE INDEX messages_undelivered ON messages (recipient_run_id) WHERE delivered_at IS NULL;
    ",
    "
    ALTER TABLE tasks ADD COLUMN evidence TEXT;
    ",
    "
    ALTER TABLE sessions ADD COLUMN name TEXT;
    ALTER TABLE sessions ADD COLUMN ord INTEGER;
    ALTER TABLE sessions ADD COLUMN build TEXT;
    ALTER TABLE sessions ADD COLUMN wake TEXT;
    CREATE UNIQUE INDEX sessions_name ON sessions (repo_dir, name)
        WHERE name IS NOT NULL;
    ",
    "
    ALTER TABLE sessions ADD COLUMN pane TEXT;
    CREATE TABLE handoffs (
        repo_dir TEXT NOT NULL,
        issue INTEGER NOT NULL,
        runtime TEXT NOT NULL,
        from_run_id TEXT,
        created_at TEXT NOT NULL,
        projection TEXT NOT NULL CHECK (projection IN ('landed', 'deferred')),
        PRIMARY KEY (repo_dir, issue, runtime)
    );
    ",
    "
    CREATE TABLE taken_decisions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        run_id TEXT NOT NULL,
        question TEXT NOT NULL,
        decision TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        reported_at INTEGER
    );
    CREATE INDEX taken_decisions_unreported ON taken_decisions (run_id) WHERE reported_at IS NULL;
    ",
    "
    CREATE TABLE transitions (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        repo_id INTEGER NOT NULL REFERENCES repos(id),
        issue INTEGER NOT NULL,
        at TEXT NOT NULL,
        run_id TEXT,
        from_state TEXT,
        to_state TEXT NOT NULL CHECK (to_state IN
            ('analysis', 'ready', 'in-progress', 'review', 'blocked', 'done'))
    );
    CREATE INDEX transitions_issue ON transitions (repo_id, issue, seq);
    CREATE TRIGGER transitions_no_update BEFORE UPDATE ON transitions
        WHEN NOT (
            NEW.issue <> OLD.issue
            AND EXISTS (SELECT 1 FROM provisional_issues
                        WHERE repo_id = OLD.repo_id AND provisional = OLD.issue
                          AND real = NEW.issue)
            AND NEW.seq IS OLD.seq AND NEW.repo_id IS OLD.repo_id
            AND NEW.at IS OLD.at AND NEW.run_id IS OLD.run_id
            AND NEW.from_state IS OLD.from_state AND NEW.to_state IS OLD.to_state)
        BEGIN SELECT RAISE(ABORT, 'append-only'); END;
    CREATE TRIGGER transitions_no_delete BEFORE DELETE ON transitions
        BEGIN SELECT RAISE(ABORT, 'append-only'); END;
    ",
];
