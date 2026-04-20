# Documentation Map

This repository keeps current product docs, contributor docs, and historical plans side by side. Use this page as the entry point.

## Current Docs

- [README.md](../README.md): product overview, install paths, local run flow, desktop packaging, remote viewer basics
- [README.zh.md](../README.zh.md): Chinese version of the user-facing product docs
- [CONTRIBUTING.md](../CONTRIBUTING.md): contributor setup, architecture overview, conventions, adapter integration checklist
- [AGENTS.md](../AGENTS.md) and [CLAUDE.md](../CLAUDE.md): condensed repo context for coding agents working in this workspace
- [simplification-plan-2026-04-15.md](./simplification-plan-2026-04-15.md): scope decision log for the current product boundary

## Runtime Surfaces

- Local admin API and bundled web shell: `127.0.0.1:46321`
- Web dev server: `127.0.0.1:4173`
- Remote read-only viewer: `0.0.0.0:46322`, only when enabled from `Settings -> Remote Access`

## Historical Docs

Superseded design notes, implementation plans, and older visual explorations live under [`docs/history/`](./history/).

These files are still useful for rationale, but they are not the source of truth for current behavior.

## Maintenance Rules

- Avoid exact test counts or other fast-drifting status snapshots in long-lived docs.
- When a plan is completed or abandoned, move it under `docs/history/` instead of leaving it in the main `docs/` root.
- Keep `README.md` and `README.zh.md` aligned for user-facing behavior.
- If a historical note conflicts with current behavior, prefer `README.md`, `README.zh.md`, and `CONTRIBUTING.md`.
