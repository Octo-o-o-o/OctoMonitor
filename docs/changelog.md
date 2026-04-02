# Changelog

## Unreleased
- Added probe-driven live telemetry ingestion to the Rust server, including Claude/Codex ingest endpoints and periodic local adapter refresh.
- Added Playwright + axe-core accessibility audit coverage for wallboard/history/setup/companion/e-ink routes and fixed the e-ink contrast issue.
- Added a buildable Tauri desktop release path, including workspace wiring, `build.rs`, icon assets, and archived workflow artifact output.
- Added pnpm + Rust workspace scaffold for OctoMonitor.
- Implemented demo-backed Axum local server with bootstrap, health, config, installer, pairing, and stream endpoints.
- Added React wallboard/history/setup/companion/e-ink UI with inspect drawer and Zustand state.
- Added installer/companion/adapter/core crates and minimal Tauri 2 desktop shell scaffold.
