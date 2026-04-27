# Contributing to OctoMonitor

Thanks for your interest in OctoMonitor! This guide will help you get up and running.

Current user-facing product docs live in [README.md](README.md), [README.zh.md](README.zh.md), and [docs/README.md](docs/README.md). Historical plans and superseded design notes live under `docs/history/`.

## Development Setup

### Prerequisites

- **Rust** 1.75+ via [rustup](https://rustup.rs/)
- **Node.js** 20+ with **pnpm** 10+
- At least one supported tool installed (Claude Code, Codex, OpenClaw, or Hermes in experimental mode)

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
                    │     (crates/server)          │
                    │ local API + remote viewer    │
                    │ probe · watcher · handlers/* │
                    └──┬────────┬────────┬────────┘
                       │        │        │
              ┌────────▼┐  ┌────▼───┐  ┌─▼────────┐  ┌────────▼┐
              │ Claude   │  │ Codex  │  │ OpenClaw │  │ Hermes   │
              │ Adapter  │  │ Adapter│  │ Adapter  │  │ Adapter  │
              └──────────┘  └────────┘  └──────────┘  └──────────┘
                 reads          reads        reads         reads
              JSONL logs    JSONL logs   sessions +    sessions +
                                         gateway data  gateway data
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `core` | Domain types (`RunRecord`, `BootstrapPayload`, history payloads, remote access state), `ts-rs` exports |
| `server` | Axum local API, read-only remote viewer surface, config persistence, probe refresh, file watching, route handlers, WebSocket streaming |
| `adapters/common` | Shared adapter utilities such as path resolution and command/file probe helpers |
| `adapters/claude` | Parses Claude Code JSONL session logs |
| `adapters/codex` | Parses Codex JSONL session logs |
| `adapters/openclaw` | Parses OpenClaw sessions, gateway state, and cron metadata |
| `adapters/hermes` | Parses Hermes sessions, gateway state, profiles, and cron metadata |
| `installer` | Tool detection and doctor diagnostics only; it does not rewrite tool config files |
| `companion` | Pairing codes and cookie-backed remote viewer sessions |

### Data Flow

1. **Bootstrap refresh**: server startup uses a blocking scan across all four adapters to build the initial snapshot
2. **Background updates**: active runs refresh every 30s, idle state refreshes every 120s, and file-system or ingest events can wake the probe immediately
3. **State update**: probe results merge into `AppState.bootstrap` via `tokio::sync::RwLock`; derived history and commit views refresh separately
4. **Broadcast**: `state.signal_change()` notifies local UI and paired remote viewers
5. **WebSocket push**: clients receive `snapshot.replace` frames over `/api/stream`; remote viewers get the redacted variant on port `46322`
6. **Frontend**: Zustand store replaces data atomically; remote viewer mode hides local-only controls

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
│   │   ├── CommitsView.tsx    # Recent commit history + attribution
│   │   ├── HeatmapView.tsx    # Historical activity heatmap + local summary
│   │   ├── SettingsView.tsx   # Settings (delegates to settings/*)
│   │   ├── StatusBar.tsx      # Top bar with WS status + stats
│   │   ├── Skeleton.tsx       # Loading skeletons
│   │   ├── AttentionBanner.tsx
│   │   ├── DateRangePicker.tsx
│   │   └── settings/          # Settings sections including remote access
│   ├── InspectDrawer.tsx      # Session detail side panel
│   ├── RemotePairingGate.tsx  # Remote viewer unlock flow
│   ├── LoadingScreen.tsx      # Desktop/web boot and reconnect states
│   ├── ErrorBoundary.tsx      # Crash recovery
│   └── ShortcutOverlay.tsx    # Keyboard shortcut help
├── lib/
│   ├── types.ts         # Re-exports ts-rs bindings from crates/core
│   ├── i18n.tsx         # Compile-time safe i18n (en/zh)
│   ├── api.ts           # Same-origin fetch helpers + WS URL builder
│   ├── format.ts        # Shared formatters
│   ├── monitor.ts       # Visible-run selectors shared by UI and shortcuts
│   ├── preferences.ts   # Frontend preference schema + migration
│   ├── runtimeMode.ts   # local vs remoteViewer runtime split
│   ├── desktopEvents.ts # Native desktop event names
│   ├── storageKeys.ts   # Browser storage key registry
│   ├── i18nMaps.ts      # Typed label maps for enum-backed strings
│   └── theme.tsx        # Theme management + VS Code import
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
- Local admin APIs stay on loopback; the remote viewer is a separate opt-in read-only surface
- Config files go in `~/.octomonitor/`

## Pull Request Guidelines

1. **One concern per PR** — don't mix refactors with features
2. **Tests required** — add/update tests for any logic changes
3. **CI must pass** — `cargo test`, `pnpm test`, `cargo clippy`, `pnpm build`
4. **No new runtime JS deps** without discussion — the lean dep count is intentional
5. **i18n** — add both `en` and `zh` keys for any new user-facing strings

## Adding a New Adapter

1. Create a new crate in `crates/adapters/<name>/`
2. Implement `descriptor()`, `probe()`, and any cache helpers your adapter needs
3. Add the tool to `ToolKind` in [`crates/core/src/lib.rs`](crates/core/src/lib.rs)
4. Wire the adapter into [`crates/server/src/probe.rs`](crates/server/src/probe.rs) and [`crates/server/src/watcher.rs`](crates/server/src/watcher.rs) if it has watchable local state
5. Extend installer detect/doctor output if the tool should appear in Environment & Doctor
6. Update frontend constants, default panel/filter settings, and i18n labels for the new tool

## Questions?

Open an issue on GitHub — we're happy to help!
