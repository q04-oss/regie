-- =============================================================
-- Régie v1 — Phase 3: persist CLAUDE.md alongside the repo row.
--
-- Lets `regenerate_recommendation` read the briefing document
-- straight out of the DB instead of re-hitting the GitHub API on
-- every POST /api/repos/:repo_id/recommendation. The column is
-- nullable — older repos predating this column, and repos whose
-- CLAUDE.md doesn't exist on GitHub, both store NULL.
-- =============================================================

ALTER TABLE repos
    ADD COLUMN IF NOT EXISTS claude_md TEXT;
