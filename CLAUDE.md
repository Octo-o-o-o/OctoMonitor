# CLAUDE.md

## Project
- Name: OctoMonitor
- Goal: local-first unified monitor for Claude Code / Codex / OpenClaw
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
- `crates/installer`: detect/doctor probes
- `crates/companion`: pairing/session logic

## Current State
- `cargo test --workspace` passes (18 tests)
- `pnpm --filter @octomonitor/web test --run` passes
- `pnpm --filter @octomonitor/web build` passes
- `pnpm test:a11y` passes (reports saved to `apps/web/test-results/`)
- `pnpm build:desktop` builds web + server + desktop (server binary is built first, desktop finds it at sibling path in release)
- `cargo run -p octomonitor-server` exposes `/api/bootstrap`, `/api/health`, `/api/config`, installer APIs, pairing APIs, live ingest APIs, and `/api/stream`
- Server bootstrap blends local adapter probes with in-memory live ingest updates; config patches survive probe refresh cycles
- Web UI has WS reconnect with exponential backoff and a LIVE/OFFLINE status indicator
- Desktop finds the server binary relative to itself in release builds; only falls back to `cargo run` in debug builds

## Rules
- No database
- Read-only by default
- Gateway/official APIs beat derived file scans
- Never expose secrets or render raw tokens
- WS is the primary live channel; Tauri event is not
- Companion-safe API access must use same-origin/host-relative URLs rather than hardcoded loopback URLs
