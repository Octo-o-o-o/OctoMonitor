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

## CLI

OctoMonitor includes a CLI (`octomonitor`) for managing workflows programmatically — especially useful for LLM-driven automation where an AI agent reads a spec and creates workflows via the terminal.

```bash
# Build the CLI
cargo build -p octomonitor-cli

# Print a workflow JSON template (shows all fields and variable docs)
octomonitor workflow template > my-workflow.json

# Create a workflow from a JSON file
octomonitor workflow create -f my-workflow.json

# Create and immediately start a run
octomonitor workflow create -f my-workflow.json --run --dir /path/to/project --mode assisted

# List definitions and runs
octomonitor wf list
octomonitor wf runs

# Start a run from an existing definition
octomonitor wf run <workflow-id> -d /path/to/project -m auto

# Inspect a run (shows step states, linked runs, etc.)
octomonitor wf inspect <run-id>

# Step operations
octomonitor wf step <run-id> step-0 approve
octomonitor wf step <run-id> step-0 complete
octomonitor wf step <run-id> step-0 skip

# Link a monitor run to a step
octomonitor wf link <run-id> step-0 <monitor-run-id>

# Read from stdin (LLM can pipe JSON directly)
echo '{ ... }' | octomonitor wf create -f -

# Custom server URL
octomonitor --server http://192.168.1.100:46321 wf list
# Or via environment variable
export OCTOMONITOR_URL=http://192.168.1.100:46321
```

The CLI talks to the running `octomonitor-server` over HTTP (default `http://127.0.0.1:46321`). Make sure the server is running before using CLI commands.

## Architecture

```
OctoMonitor
├── crates/
│   ├── core/            # Domain types, ts-rs exports
│   ├── server/          # Axum local API + remote read-only viewer surface
│   ├── cli/             # CLI for workflow management (octomonitor binary)
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
