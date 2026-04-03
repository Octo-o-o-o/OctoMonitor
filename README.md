# OctoMonitor

[中文说明](README.zh.md)

**Local-first unified monitor for Claude Code, Codex, and OpenClaw.**

OctoMonitor gives you a single dashboard to watch all your AI coding sessions in real time — token usage, quota, costs, and session state — without sending any data to the cloud.

![OctoMonitor Screenshot](exports/octomonitor-figma-designer-handoff-2026-04-01/previews/desktop-preview.png)

## Features

- **Three-tool unified view** — Claude Code, Codex, and OpenClaw sessions in one place
- **Real-time WebSocket updates** — event-driven push, no polling
- **Token & cost tracking** — per-session and aggregated usage with quota bars
- **Desktop notifications** — get notified when a session needs approval
- **Keyboard-driven** — `j`/`k` navigation, `1`/`2`/`3` tab switching, `?` shortcut help
- **Dark / Light / E-Ink themes** — plus VS Code theme import
- **Local-first** — all data stays on your machine; server binds to `127.0.0.1`
- **Zero-config** — auto-detects installed tools; no database required
- **Remote viewer** — opt-in read-only companion surface for LAN / private-network devices
- **i18n** — English and Chinese, compile-time safe

## Install

For end users who want a packaged build instead of running from source:

- **macOS desktop app** — download the notarized `.dmg` from [GitHub Releases](https://github.com/Octo-o-o-o/OctoMonitor/releases)
- **Homebrew service / local server** — `brew install Octo-o-o-o/octomonitor/octomonitor`
- **npm package** — `npm install -g octomonitor` or `npx octomonitor`

The Homebrew and npm packages install the local `octomonitor-server` binary. The npm release line now covers macOS, Linux x64, and Windows x64. The desktop `.dmg` is distributed separately through GitHub Releases and remains macOS-only. If you prefer the short Homebrew name later, run `brew tap Octo-o-o-o/octomonitor` first and then use `brew install octomonitor`.

## Quickstart

### Prerequisites

- [Rust](https://rustup.rs/) (1.75+)
- [Node.js](https://nodejs.org/) (20+) with [pnpm](https://pnpm.io/) (10+)
- At least one of: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [OpenClaw](https://github.com/openclaw)

### Run (Web)

```bash
# Clone
git clone https://github.com/Octo-o-o-o/OctoMonitor.git
cd OctoMonitor

# Install JS dependencies
pnpm install

# Start the server (port 46321)
cargo run -p octomonitor-server

# In another terminal, start the web UI
pnpm --filter @octomonitor/web dev
```

Open [http://127.0.0.1:4173](http://127.0.0.1:4173). The dashboard connects via WebSocket and shows live data. If the server isn't running, the UI shows an offline state instead of fake demo data.

### Run (Desktop)

```bash
# Build an unsigned desktop release bundle locally
pnpm build:desktop
```

For a signed-only macOS bundle on a machine with a valid Developer ID certificate:

```bash
pnpm build:desktop:signed
```

For the release-grade macOS bundle with notarization and stapling:

```bash
pnpm build:desktop:notarized
```

`pnpm build:desktop:notarized` expects `APPLE_ID`, `APPLE_TEAM_ID`, and either `APPLE_PASSWORD` or `APPLE_APP_SPECIFIC_PASSWORD` in your shell environment. It signs the app, submits it to Apple with `notarytool`, staples both the `.app` and `.dmg`, and leaves the final artifacts in `target/release/bundle/`.

Or for development:

```bash
cargo tauri dev
```

## Architecture

```
OctoMonitor
├── crates/
│   ├── core/            # Domain types, ts-rs exports
│   ├── server/          # Axum local API + remote read-only viewer surface
│   ├── adapters/
│   │   ├── claude/      # Claude Code session parser
│   │   ├── codex/       # Codex session parser
│   │   └── openclaw/    # OpenClaw session parser
│   ├── installer/       # Tool detection, sandbox manifests, doctor, rollback
│   └── companion/       # Pairing codes + viewer sessions
├── apps/
│   ├── web/             # React 19 + Zustand + Vite 7 + Tailwind CSS 4
│   └── desktop/         # Tauri 2 shell
└── docs/
```

**Key design decisions:**

| Decision | Rationale |
|----------|-----------|
| Rust server + React frontend | Browser can't read local files; a local process is necessary |
| Tauri 2 desktop shell | Light, reuses web UI, avoids Electron bloat |
| No database | Monitor reads tool files as source of truth |
| 3 runtime JS deps | `react`, `react-dom`, `zustand` — intentionally lean |
| WebSocket-only data flow | Event-driven push eliminates polling and race conditions |
| `tokio::sync::RwLock` | Async-safe, read-many/write-few friendly |
| Parallel adapter probing | `std::thread::scope` for concurrent blocking I/O |
| Split local/remote surfaces | Full admin APIs stay loopback-only; remote viewer is read-only + cookie-authenticated |

## Development

```bash
# Run all Rust tests
cargo test --workspace

# Lint Rust
cargo clippy --workspace -- -D warnings

# Run web tests
pnpm --filter @octomonitor/web test --run

# Run accessibility audit
pnpm test:a11y

# Run the full pre-release gate
pnpm release:check

# Build a release-grade notarized macOS package
pnpm build:desktop:notarized
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch tab (Monitor / Usage / Settings) |
| `j` / `k` | Navigate session list |
| `Enter` | Open detail drawer |
| `Esc` | Close drawer |
| `?` | Show shortcut overlay |

## Configuration

Server-side remote access state is persisted to `~/.octomonitor/config.json` and survives restarts. Frontend display preferences (theme, density, filters, notifications) are stored in `localStorage`.

The Setup screen currently exposes diagnostics plus local sandbox manifest helpers. It does not automatically rewrite Claude Code, Codex, or OpenClaw configuration files.

## License

[MIT](LICENSE)
