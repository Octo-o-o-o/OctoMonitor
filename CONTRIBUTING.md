# Contributing to OctoMonitor

Thanks for your interest in OctoMonitor! This guide will help you get up and running.

## Development Setup

### Prerequisites

- **Rust** 1.75+ via [rustup](https://rustup.rs/)
- **Node.js** 20+ with **pnpm** 10+
- At least one supported tool installed (Claude Code, Codex, or OpenClaw)

### First-time setup

```bash
git clone https://github.com/Octo-o-o-o/OctoMonitor.git
cd OctoMonitor
pnpm install
cargo build
```

### Running locally

```bash
# Terminal 1: start the server
cargo run -p octomonitor-server

# Terminal 2: start the web dev server
pnpm --filter @octomonitor/web dev
```

### Running tests

```bash
cargo test --workspace        # Rust tests
cargo clippy --workspace -- -D warnings
pnpm --filter @octomonitor/web test --run
pnpm test:a11y                # Accessibility audit (Playwright + axe-core)
pnpm build:desktop            # Unsigned desktop bundle smoke check
pnpm build:desktop:signed     # Signed-only macOS bundle
pnpm build:desktop:notarized  # Signed + notarized + stapled macOS release bundle
pnpm release:check            # Full pre-release gate
```

## Architecture Overview

```
                    ┌─────────────────────────────┐
                    │     Tauri 2 Desktop Shell    │
                    │    (apps/desktop/src-tauri)  │
                    └──────────┬──────────────────┘
                               │ embeds
                    ┌──────────▼──────────────────┐
                    │    React 19 + Zustand UI     │
                    │        (apps/web)            │
                    │  Vite 7 · Tailwind CSS 4     │
                    └──────────┬──────────────────┘
                               │ WebSocket
                    ┌──────────▼──────────────────┐
                    │      Axum HTTP/WS Server     │
                    │     (crates/server)           │
                    │  state · config · probe ·     │
                    │  handlers/* · static_files    │
                    └──┬───────┬───────┬───────────┘
                       │       │       │  adapter trait
              ┌────────▼┐  ┌──▼────┐  ┌▼──────────┐
              │ Claude   │  │ Codex │  │ OpenClaw   │
              │ Adapter  │  │Adapter│  │ Adapter    │
              └──────────┘  └───────┘  └───────────┘
                 reads          reads       reads
              JSONL logs    JSONL logs   session files
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `core` | Domain types (`RunRecord`, `BootstrapPayload`, etc.), `ts-rs` exports, demo data |
| `server` | Axum server: state management, config persistence, probe refresh loop, route handlers, WebSocket streaming |
| `adapters/claude` | Parses Claude Code JSONL session logs |
| `adapters/codex` | Parses Codex JSONL session logs |
| `adapters/openclaw` | Parses OpenClaw session files and API |
| `installer` | Tool detection, install planning, doctor checks, rollback |
| `companion` | Pairing codes and cookie-backed remote viewer sessions |

### Data Flow

1. **Probe refresh** (every 15s): `std::thread::scope` runs all three adapter probes in parallel
2. **State update**: probe results merged into `AppState.bootstrap` via `tokio::sync::RwLock`
3. **Broadcast**: `state.signal_change()` sends a notification through `tokio::sync::broadcast`
4. **WebSocket push**: local and remote read-only clients receive a fresh `snapshot.replace` frame
5. **Frontend**: Zustand store replaces data atomically; React re-renders affected components

### Frontend Structure

```
apps/web/src/
├── App.tsx              # Root: WS hook, keyboard shortcuts, tab routing
├── main.tsx             # Entry point with ErrorBoundary + I18nProvider
├── store/
│   └── monitorStore.ts  # Zustand store (single setData pattern)
├── components/
│   ├── monitor/
│   │   ├── MonitorView.tsx    # Three-column session dashboard
│   │   ├── UsageView.tsx      # Token/cost analytics
│   │   ├── SettingsView.tsx   # Settings (delegates to settings/*)
│   │   ├── StatusBar.tsx      # Top bar with WS status + stats
│   │   ├── Skeleton.tsx       # Loading skeletons
│   │   ├── AttentionBanner.tsx
│   │   ├── DateRangePicker.tsx
│   │   └── settings/          # Settings sections including remote access
│   ├── InspectDrawer.tsx      # Session detail side panel
│   ├── RemotePairingGate.tsx  # Remote viewer unlock flow
│   ├── ErrorBoundary.tsx      # Crash recovery
│   └── ShortcutOverlay.tsx    # Keyboard shortcut help
├── lib/
│   ├── types.ts         # Re-exports ts-rs bindings from crates/core
│   ├── i18n.tsx         # Compile-time safe i18n (en/zh)
│   ├── format.ts        # Shared formatTokens/formatCost
│   ├── monitor.ts       # Visible-run selectors shared by UI and shortcuts
│   ├── preferences.ts   # Frontend preference schema + migration
│   ├── runtimeMode.ts   # local vs remoteViewer runtime split
│   ├── theme.tsx        # Theme management + VS Code import
│   └── mockData.ts      # Test fixtures
└── styles.css           # Global styles + Tailwind @theme tokens
```

## Conventions

### Rust

- Use `tokio::sync::RwLock` for shared async state (never `std::Mutex`)
- Server binds to `127.0.0.1` by default — never `0.0.0.0`
- No database; all state is in-memory or read from tool files
- Adapter probes must be blocking-safe (run in `std::thread::scope`)
- `cargo clippy -- -D warnings` must pass

### TypeScript / React

- Only 3 runtime dependencies: `react`, `react-dom`, `zustand`
- No React Router — three tabs don't need it
- No component library — custom CSS with Tailwind utility classes
- All user-facing strings go through the i18n system
- Adding an `en` key without a `zh` translation is a compile error
- Prefer Zustand selectors over hooks that subscribe to the entire store
- Runtime mode matters: remote viewers must not surface local-only settings or mutating controls

### General

- Never expose secrets or render raw tokens
- WebSocket is the primary live channel; no Tauri events for data flow
- Gateway/official APIs beat derived file scans
- Config files go in `~/.octomonitor/`

## Pull Request Guidelines

1. **One concern per PR** — don't mix refactors with features
2. **Tests required** — add/update tests for any logic changes
3. **CI must pass** — `cargo test`, `pnpm test`, `cargo clippy`, `pnpm build`
4. **No new runtime JS deps** without discussion — the lean dep count is intentional
5. **i18n** — add both `en` and `zh` keys for any new user-facing strings

## Adding a New Adapter

1. Create a new crate in `crates/adapters/<name>/`
2. Implement a `probe()` function that returns a snapshot struct
3. Add the tool to `ToolKind` enum in `crates/core/src/lib.rs`
4. Wire the probe into `crates/server/src/probe.rs` inside `std::thread::scope`
5. Add a panel entry in the frontend store's `defaultPanelConfig`
6. Add i18n keys for the new tool name

## Questions?

Open an issue on GitHub — we're happy to help!
