# Régie

Internal engineering intelligence dashboard for the q04-oss ecosystem.

Régie reads connected repositories and surfaces codebase health, scorecard
trends, deferred items, and AI-powered recommendations into a unified
interface.

## What Régie is not

Régie does not replace Grafana (metrics), Sentry (errors), Loki (logs), or
Metabase (SQL analytics). Those tools handle runtime observability. Régie
handles engineering intelligence — what the codebase is, where it stands,
and what needs to happen next.

## v1 Features

1. **Scorecard trend** — reads `SCORECARD.md` from connected repos via the
   GitHub API. Renders grade history as a trend chart.
2. **Deferred items board** — reads `HARDENING.md`, extracts deferred items,
   renders as a prioritised board with file refs.
3. **Commit semantic summaries** — GitHub API for recent commits. Claude API
   summarises each commit in plain language — what changed and why it matters.
4. **What to work on next** — Claude reads scorecard + deferred items +
   recent commits and recommends the highest-leverage next action with
   justification.
5. **Cross-repo state** — same pipeline applied to every q04-oss repo that
   has a `CLAUDE.md`. Single unified view across all products.
6. **Task creation** — create a task in Régie. It becomes a structured
   prompt that Claude Code picks up in the next session.

## Integrations

- GitHub API — repository reading, commit history
- Anthropic Claude API — semantic summaries, recommendations
- Links to Grafana, Sentry, Metabase in context

## Stack

- API: Rust / Axum
- Frontend: Rust / Leptos (CSR, WASM)
- Database: PostgreSQL (project state, task history)
- AI: Anthropic Claude API

## Quick start

```
cargo build
cargo run -p regie-api
```

## Repo structure

```
api/      — Axum REST/JSON API
web/      — Leptos CSR frontend (WASM)
shared/   — Types shared between api and web
```
