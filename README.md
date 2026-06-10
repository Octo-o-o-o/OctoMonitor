# OctoMonitor

[中文说明](README.zh.md) · [Documentation Map](docs/README.md)

**Local-first unified monitor for Claude Code, Codex, OpenClaw, and Hermes (experimental).**

OctoMonitor gives you a single dashboard to watch all your AI coding sessions in real time — token usage, quota, costs, and session state — without sending any data to the cloud.

## Features

- **Unified local monitor** — `Monitor / Usage / Commits / Heatmap / Settings` in one app
- **Real-time WebSocket updates** — event-driven push, no polling
- **Token & cost tracking** — per-session and aggregated usage with quota bars
- **Commit and heatmap views** — practical history views without a separate analytics product layer
- **Desktop notifications** — get notified when a session needs approval
- **Keyboard-driven** — app shortcuts plus native desktop actions for Preferences, zoom, and standard edit commands
- **Dark / Light / E-Ink themes** — plus VS Code theme import
- **Local-first** — all data stays on your machine; server binds to `127.0.0.1`
- **Zero-config** — auto-detects installed tools; no database required
- **Read-only remote viewer** — opt-in companion surface for paired LAN / private-network devices, limited to `Monitor / Usage`
- **Hermes adapter (experimental)** — still visible in monitor/usage flows, but not a first-class product track
- **i18n** — English and Chinese, compile-time safe

## Integration Support

Reviewed against official docs and upstream GitHub sources on 2026-06-10. See the detailed notes in [docs/integration-support-audit-2026-06-10.md](docs/integration-support-audit-2026-06-10.md).

| CLI | Level | What OctoMonitor can do today |
|-----|-------|--------------------------------|
| Claude Code | Monitored | Count and display sessions, tokens, cost, state, workspace, and transcript-derived detail through local transcript scans plus statusline/hook ingest paths. Copy-resume is available when a session id is present; no mutating operation bridge is enabled. |
| Codex | Monitored | Count and display local sessions, token usage, state, Codex event timelines, resume commands, and desktop deep links where a thread id is available. Codex hooks now use `[features].hooks` as the canonical config key. |
| OpenClaw | Monitored | Count and display Gateway/session-store state, usage, sessions, and health-style source status through the existing adapter. Operations remain read-only. |
| Hermes | Experimental | Count and display Hermes CLI/Gateway sessions from local state and profile-aware scans. Copy-resume is available when profile/session metadata is present, but the adapter remains intentionally experimental. |
| Gemini CLI / CodeBuddy / Pi Agent | Experimental, fixture-gated | Passive local scans can count sessions and usage from locked fixture schemas when those stores exist. Hook Manager supports opt-in hook installation for Gemini and CodeBuddy. These adapters are not marked stable and must not read OAuth tokens or provider secrets. |
| opencode / GitHub Copilot / OpenHands / Continue cn / Qwen Code / Kimi Code / Goose | Experimental, fixture-gated | Passive scans cover known local stores where available. They expose source health, usage semantics, and safe copy/open capabilities only; no approve/deny/kill/send operations are enabled. |
| Cursor Agent | Experimental opt-in | Private-store parsing is disabled unless `OCTOMONITOR_CURSOR_PRIVATE_STORE=1` is set. It can display session metadata, but token/cost usage is `N/A` because the upstream local store does not include usage. |
| Cline / Kiro | Experimental metadata | Fixture-gated metadata/custom-storage parsing only. Usage remains `N/A` unless an explicit usage field is present; no operation bridge is enabled. |
| WorkBuddy / Amazon Q / Aider / Amp / Windsurf / Codebuff / Roo / Kilo | Detection-only / watchlist | Displayed for source control and future research only. They are not treated as stable monitored sources and do not contribute usage unless a future adapter graduates with fixtures. |

## Install

For end users who want a packaged build instead of running from source:

- **macOS desktop app** — download the notarized `.dmg` from [GitHub Releases](https://github.com/Octo-o-o-o/OctoMonitor/releases)
- **Homebrew service / local server** — `brew install Octo-o-o-o/octomonitor/octomonitor`
- **npm package** — `npm install -g octomonitor` or `npx octomonitor`

The Homebrew and npm packages install an `octomonitor` command backed by the local server binary. The npm release line targets macOS, Linux x64, and Windows x64; today only `octomonitor-darwin-arm64` is published on npm — the other platform binaries will follow once the v0.1.6 release workflow runs. The desktop `.dmg` is distributed separately through GitHub Releases and remains macOS-only. If you prefer the short Homebrew name later, run `brew tap Octo-o-o-o/octomonitor` first and then use `brew install octomonitor`.

## Quickstart

### Prerequisites

- [Rust](https://rustup.rs/) (1.75+)
- [Node.js](https://nodejs.org/) (20+) with [pnpm](https://pnpm.io/) (10+)
- At least one monitored or experimental source: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [OpenClaw](https://github.com/openclaw), Hermes (experimental), or one of the fixture-gated passive adapters above.

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

## Remote Viewer

OctoMonitor keeps the full admin surface on `127.0.0.1:46321`. Remote access is opt-in and starts a separate read-only viewer only when you enable it.

1. Open `Settings -> Remote Access` in the local app or localhost web UI.
2. Enable remote access. OctoMonitor starts the paired viewer on port `46322` and shows advertised LAN / private-network addresses.
3. Generate a pairing code, open one of the advertised addresses on another device, and enter the code there.
4. Paired viewers are limited to `Monitor / Usage` and use a short-lived cookie session.

## Architecture

```
OctoMonitor
├── crates/
│   ├── core/            # Domain types, ts-rs exports
│   ├── server/          # Axum local API + remote read-only viewer surface
│   ├── adapters/
│   │   ├── claude/      # Claude Code session parser
│   │   ├── codex/       # Codex session parser
│   │   ├── openclaw/    # OpenClaw session parser
│   │   └── hermes/      # Hermes session parser (experimental)
│   ├── installer/       # Tool detection + doctor diagnostics
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
| Parallel adapter probing | Isolated concurrent probe tasks keep local scans parallel without adding a database |
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
| `1` / `2` / `3` / `4` / `5` | Switch tab (Monitor / Usage / Commits / Heatmap / Settings) |
| `j` / `k` | Navigate session list |
| `Enter` | Open detail drawer |
| `Esc` | Close drawer |
| `?` | Show shortcut overlay |
| `Cmd` / `Ctrl` + `,` | Open Settings in the desktop app |
| `Cmd` / `Ctrl` + `+` / `-` / `0` | Zoom in / out / reset in the desktop app |

Desktop builds also support native undo, redo, cut, copy, paste, and select-all shortcuts through the system menu bar.

## Configuration

The local admin surface stays on `127.0.0.1:46321`. If remote access is enabled, OctoMonitor also starts a separate read-only viewer on `0.0.0.0:46322` and advertises reachable URLs in Settings. Server-side remote access state is persisted to `~/.octomonitor/config.json` and survives restarts. Frontend display preferences (theme, density, filters, notifications) are stored in `localStorage`.

The Environment / Doctor screen exposes detection and diagnostics only. It does not silently rewrite Claude Code, Codex, OpenClaw, Hermes, Gemini, Cursor Agent, WorkBuddy/CodeBuddy, or Pi configuration files. Hook Manager writes only after an explicit preview/apply flow with backup, verification, uninstall, and audit. Detection-only/watchlist integrations may appear in Settings, but they do not contribute usage unless a fixture-gated or monitored adapter emits runs with non-`N/A` usage semantics.

## License

[MIT](LICENSE)
