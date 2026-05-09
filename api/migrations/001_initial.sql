-- =============================================================
-- Régie v1 — Phase 2: persistence schema
--
-- Six tables: tracked repos and the four streams of state we
-- pull off them (scorecard history, deferred items, commit
-- summaries, recommendations) plus the task log. Repo-id is the
-- canonical "owner/name" form (e.g. "q04-oss/box-fraise-platform")
-- and is the foreign key everywhere else.
-- =============================================================

CREATE TABLE IF NOT EXISTS repos (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    grade             TEXT,
    test_count        INTEGER,
    last_commit_sha   TEXT,
    last_commit_at    TIMESTAMPTZ,
    last_ingested_at  TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS scorecard_entries (
    id                     SERIAL PRIMARY KEY,
    repo_id                TEXT NOT NULL REFERENCES repos(id),
    entry_date             DATE NOT NULL,
    grade                  TEXT NOT NULL,
    weighted_score         DOUBLE PRECISION NOT NULL DEFAULT 0,
    security               DOUBLE PRECISION NOT NULL DEFAULT 0,
    architecture           DOUBLE PRECISION NOT NULL DEFAULT 0,
    engineer_usability     DOUBLE PRECISION NOT NULL DEFAULT 0,
    protocol_conformance   DOUBLE PRECISION NOT NULL DEFAULT 0,
    operational_readiness  DOUBLE PRECISION NOT NULL DEFAULT 0,
    product_completeness   DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (repo_id, entry_date)
);

CREATE INDEX IF NOT EXISTS idx_scorecard_entries_repo_date
    ON scorecard_entries(repo_id, entry_date DESC);

CREATE TABLE IF NOT EXISTS deferred_items (
    id           TEXT NOT NULL,
    repo_id      TEXT NOT NULL REFERENCES repos(id),
    description  TEXT NOT NULL,
    file_ref     TEXT,
    section      TEXT,
    priority     TEXT NOT NULL DEFAULT 'medium',
    resolved     BOOLEAN NOT NULL DEFAULT false,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (repo_id, id)
);

CREATE INDEX IF NOT EXISTS idx_deferred_items_repo_unresolved
    ON deferred_items(repo_id) WHERE resolved = false;

CREATE TABLE IF NOT EXISTS commit_summaries (
    sha               TEXT NOT NULL,
    repo_id           TEXT NOT NULL REFERENCES repos(id),
    message           TEXT NOT NULL,
    author            TEXT NOT NULL,
    committed_at      TIMESTAMPTZ NOT NULL,
    semantic_summary  TEXT,
    files_changed     INTEGER,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (repo_id, sha)
);

CREATE INDEX IF NOT EXISTS idx_commit_summaries_repo_committed_at
    ON commit_summaries(repo_id, committed_at DESC);

CREATE TABLE IF NOT EXISTS tasks (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repo_id       TEXT NOT NULL REFERENCES repos(id),
    title         TEXT NOT NULL,
    prompt        TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tasks_repo_status
    ON tasks(repo_id, status);

CREATE TABLE IF NOT EXISTS recommendations (
    id                      SERIAL PRIMARY KEY,
    repo_id                 TEXT NOT NULL REFERENCES repos(id),
    generated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    top_action              TEXT NOT NULL,
    justification           TEXT NOT NULL,
    estimated_impact        TEXT NOT NULL,
    related_deferred_items  TEXT[] NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_recommendations_repo_generated_at
    ON recommendations(repo_id, generated_at DESC);
