# OctoMonitor Design

> 2026-04-16 update: this is a historical pre-simplification design note. The current shipped scope is defined by `README.md` and `docs/simplification-plan-2026-04-15.md`.
> 2026-04-20 update: this file is retained for historical context only. Current documentation entry points are `README.md`, `README.zh.md`, `CONTRIBUTING.md`, and `docs/README.md`.

## Product Goal
Build a local-first mission control dashboard for Claude Code, Codex, and OpenClaw across desktop, localhost web, and companion read-only layouts.

## Core Features
- Wallboard with active runs, identity strip, attention queue, usage analyzer, recent completions, source health
- History page with today / 7d slicing and dimension drill-down
- Environment / Doctor page for detection, diagnostics, and companion controls
- Companion and e-ink routes
- Local Rust HTTP + WebSocket backend with in-memory aggregation only

## Architecture
- Rust workspace for domain, adapters, server, installer, companion
- React/Vite frontend consuming `/api/bootstrap` and `/api/stream`
- Tauri desktop shell embedding the web renderer
- File watch + CLI/Gateway poll + local ingest endpoints as data sources

## Runtime Data Flow
- Bootstrap is built from real local adapter probes plus runtime ingest upserts from Claude/Codex hooks or statusline payloads.
- Server re-probes local tool state every 15 seconds and preserves live-ingested runs across refreshes.
- `/api/stream` still emits `snapshot.replace` frames every few seconds, which keeps the renderer in sync without introducing a database.

## Data Model
Primary entities:
- `RunRecord`
- `AttentionItem`
- `UsageBucket`
- `IdentityState`
- `AdapterHealth`
- `CompletionRecord`

## Persistence
Only local config/aliases/pricing/pairing JSON files. No database.

## Security
- Read-only by default
- Companion access opt-in only
- Pairing token time-limited
- Never display raw credentials

## Packaging / Validation
- Web build remains Vite-based.
- Desktop validation now includes a local Tauri release binary build.
- Accessibility validation uses Playwright + axe-core across all first-release routes.
