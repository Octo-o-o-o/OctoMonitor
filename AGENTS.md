# AGENTS.md

## Project
- Name: OctoMonitor
- Goal: local-first unified monitor for Claude Code / Codex / OpenClaw / Hermes (experimental)
- Stack: Rust workspace + React/Vite + Tauri 2

## Commands
- Install JS deps: `pnpm install`
- Install Rust deps/build: `cargo build`
- Run web dev: `pnpm --filter @octomonitor/web dev`
- Run desktop dev: `cargo tauri dev`
- Test web: `pnpm --filter @octomonitor/web test --run`
- Test rust: `cargo test --workspace`
- Build web: `pnpm --filter @octomonitor/web build`
- Run a11y audit: `pnpm test:a11y`
- Build desktop release artifact: `pnpm build:desktop`

## Structure
- `apps/web`: web UI + companion layouts + Playwright/axe audit
- `apps/desktop/src-tauri`: desktop shell scaffold + release build target
- `crates/core`: domain and aggregation
- `crates/server`: local HTTP/WS API + live probe/ingest state
- `crates/adapters/*`: source adapters
- `crates/installer`: detect/doctor probes only
- `crates/companion`: pairing/session logic

## Current State
- Local admin surface stays on `127.0.0.1:46321`; optional remote read-only viewer uses `0.0.0.0:46322` when enabled
- `cargo run -p octomonitor-server` exposes bootstrap/history/config/installer/remote/ingest/stream APIs on the local surface
- Web UI exposes `Monitor / Usage / Commits / Heatmap / Settings`; paired remote viewers expose `Monitor / Usage`
- Server bootstrap blends local adapter probes with in-memory live ingest updates; config patches survive probe refresh cycles
- Web UI has WS reconnect with exponential backoff and a LIVE/OFFLINE status indicator
- Desktop finds the server binary relative to itself in release builds; only falls back to `cargo run` in debug builds
- Current documentation entry points: `README.md`, `README.zh.md`, `CONTRIBUTING.md`, `docs/README.md`

## Rules
- No database
- Read-only by default
- Gateway/official APIs beat derived file scans
- Never expose secrets or render raw tokens
- WS is the primary live channel; Tauri event is not
- Companion-safe API access must use same-origin/host-relative URLs rather than hardcoded loopback URLs
