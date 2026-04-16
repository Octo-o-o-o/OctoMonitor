# OctoMonitor Architecture Upgrade Plan

> Status: **Approved** — 2026-03-31
> Goal: Make the project open-source-ready for discerning vibe coding users who care about both UI aesthetics and code quality.

## Current State Assessment

### What's Good (Keep)

| Decision | Why it's right |
|----------|----------------|
| Rust server + React frontend split | Browser can't read local files / run CLI; a local process is necessary |
| Tauri 2 desktop shell | Light, reuses web UI, avoids Electron bloat |
| No database, pure in-memory | Monitor reads tool files as source of truth; no persistence dependency |
| Adapter-per-tool crate pattern | Clean extension point for third-party tools |
| Only 3 runtime JS deps (react, react-dom, zustand) | Intentionally lean; fast install, small bundle |
| Custom compile-time-safe i18n | Adding an `en` key without a `zh` translation is a TS error; zero-dep |
| VS Code theme import + E-Ink detection | Killer features for the target audience |
| Accessibility pipeline (Playwright + axe-core) | Shows quality commitment |
| Zustand single `setData` pattern | Payload is small (dozens of sessions); full replacement is simple and sufficient |

### What Must Change

#### P0 — Correctness & Security (block open-source)

**1. `std::Mutex` in async context → `tokio::sync::RwLock`**

```rust
// BEFORE: blocks the entire tokio worker thread
let data = state.bootstrap.lock().expect("lock poisoned").clone();

// AFTER: async-safe, read-many/write-few friendly
let data = state.bootstrap.read().await.clone();
```

**2. `0.0.0.0` → `127.0.0.1` by default (both server and Vite dev)**

A local-first monitor tool should not expose itself to the entire network. Only open to LAN when companion mode is explicitly enabled.

**3. Config persistence**

`PATCH /api/config` currently writes only to memory — settings are lost on restart. Write to `~/.octomonitor/config.json`. This is not a "database"; it's just a config file, consistent with the project's no-DB rule.

**4. Silent demo data fallback is misleading**

When the server is unreachable, the frontend silently shows `demoData` with no indication. Must show a clear "Server offline — showing demo data" banner.

**5. Port conflict handling**

If port 46321 is already in use, the server panics with an opaque error. Detect the conflict and print a clear message (e.g., "Port 46321 in use — is another OctoMonitor instance running?"). Keep the fixed port; no configurable port for v1.

#### P1 — Code Quality (block first impression)

**6. Split `main.rs` (1,165 lines) into modules**

```
crates/server/src/
  main.rs              ← ~50 lines: startup + router
  state.rs             ← AppState, merge_runtime_state
  probe.rs             ← build_bootstrap, spawn_probe_refresh
  handlers/
    mod.rs
    bootstrap.rs
    config.rs
    ingest.rs
    installer.rs
    pairing.rs
    stream.rs
```

**7. Event-driven WebSocket + eliminate bootstrap/WS race**

Two problems in the current WS implementation:

*Problem A:* `loop { sleep(5s); send(entire_payload) }` — wastes bandwidth even when nothing changed.

*Problem B:* Race condition — the frontend fires `loadBootstrap()` (HTTP) and `useWebSocket()` simultaneously on mount. If WS connects first, the slower HTTP response can overwrite newer WS data.

Target architecture:

```
State mutation → tokio::sync::broadcast::Sender<Event>
WS handler     → on connect: send initial snapshot immediately
               → then subscribe to broadcast → send only on change
Frontend       → WS-only for data flow; remove loadBootstrap() fetch
               → keep GET /api/bootstrap for curl/debugging, not for the UI
```

Keep `snapshot.replace` as the message type (incremental diff is a future optimization).

**8. Auto-generate TS types from Rust**

Use `ts-rs` to derive TypeScript interfaces from `crates/core` structs. Run as a build step, fail CI if generated types are stale. Eliminates the manual sync of 30-field `RunRecord` between Rust and TypeScript.

**9. `build_bootstrap()` should not start from `demo_bootstrap()`**

Production code currently calls `demo_bootstrap()` first, then overlays real data. Cleaner: `build_bootstrap()` constructs an empty `BootstrapPayload`, probes fill it; `demo_bootstrap()` is only used when the frontend can't reach the server.

**10. SettingsView.tsx (459+ lines) should be split**

Extract sub-components: `AppearanceSection`, `FilterSection`, `InstallerSection`, `OpenClawAgentSettings`.

**11. Deduplicate shared utility functions**

`formatTokens()` is duplicated in `MonitorView.tsx`, `StatusBar.tsx`, and `UsageView.tsx`. Extract to `lib/format.ts`.

**12. Remove `@vitejs/plugin-legacy`**

Polyfills for Safari 10+ (2016) and Chrome 49+ (2016). The target audience uses modern browsers. Remove it along with `terser` dev dep.

**13. Add React Error Boundary**

Currently if any component throws, the entire app goes white. Add a top-level error boundary with a reload button and error message.

**14. Desktop `Cargo.toml` inherit `workspace.package` metadata**

The desktop crate is the only one that doesn't inherit workspace-level version, edition, and license. Fix for consistency.

**15. Parallel adapter probing**

```rust
// BEFORE: serial
let claude = claude_adapter::probe();
let codex = codex_adapter::probe();
let openclaw = openclaw_adapter::probe();

// AFTER: parallel via tokio
let (claude, codex, openclaw) = tokio::join!(
    tokio::task::spawn_blocking(claude_adapter::probe),
    tokio::task::spawn_blocking(codex_adapter::probe),
    tokio::task::spawn_blocking(openclaw_adapter::probe),
);
```

#### P2 — UI/UX Polish

**16. Migrate from single-file CSS (1,446 lines) to Tailwind CSS v4**

Rationale:
- 1,446 lines of unstructured CSS in one file is hard for contributors to navigate
- The target audience (vibe coders) is overwhelmingly familiar with Tailwind
- Tailwind v4 uses CSS-first config with `@theme` — works naturally with the existing CSS custom property system
- The current theme.tsx keeps setting CSS variables; Tailwind consumes them

Migration strategy:
- Install `tailwindcss` v4 and `@tailwindcss/vite`
- Move design tokens into `@theme` block in a base CSS file
- Convert components one at a time; delete corresponding CSS rules as each component is converted
- Keep the existing `ThemeProvider` and CSS variable system
- Goal: visually match the current design as closely as possible; minor spacing/rounding differences are acceptable for post-launch polish

**17. Loading skeletons**

Replace `if (!data) return null` with shimmer skeletons that mirror the three-column layout.

**18. Transition animations on data updates**

- Session card state changes: 0.2s color/opacity transition via Tailwind `transition-colors`
- New session appearance: subtle slide-in
- CSS transitions only; no animation library

**19. Keyboard shortcuts**

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch tabs (Monitor / Usage / Settings) |
| `j` / `k` | Navigate session list |
| `Enter` | Open detail drawer |
| `Esc` | Close drawer (already implemented) |
| `?` | Show shortcut overlay |

**20. Desktop notifications**

When a session enters `waitingApproval`, fire a `Notification API` notification (web) or native notification (Tauri). Respect a user preference toggle.

#### P3 — Open-Source Polish

**21. `README.md` with screenshot and quickstart**

**22. `CONTRIBUTING.md` with architecture overview**

**23. `LICENSE` file (MIT, matching Cargo.toml declaration)**

**24. CI pipeline: `cargo test`, `pnpm test`, type generation check, `cargo clippy`**

---

> 2026-04-16 update: this document predates the simplification pass. Where it mentions setup/install workflows, prefer the current product boundary in `README.md` and `docs/simplification-plan-2026-04-15.md`.

## What NOT to Do (Avoiding Over-Engineering)

| Temptation | Why skip it |
|------------|-------------|
| SSE instead of WebSocket | WS works fine; the problem is polling, not the transport |
| Full event-sourcing with granular diffs | Payload is small; full snapshot on change is fine for v1 |
| Zustand store normalization (split atoms) | Dozens of sessions, event-driven updates — full replace is simple and fast enough |
| Configurable port / port discovery | Fixed port 46321 is fine; just detect conflicts with a clear error |
| `notify` crate for filesystem watching | Live ingest hooks handle active sessions; 15s poll is fine for historical data |
| React Router / TanStack Router | Three tabs don't need a router |
| shadcn/ui or Radix | Overkill for a few toggles and cards |
| recharts / d3 for Usage page | Add later when time-series data is available |
| Setup wizard | Current detect + doctor flow is enough; no install/rollback path remains in product scope |
| `ratatui` TUI | Separate product; don't block open-source on it |
| CSS-in-JS | Tailwind is the better choice |
| Graceful shutdown with drain logic | `Ctrl+C` is fine for a local dev tool |
| Plugin system / adapter registry | Three adapters don't need it |
| `crates.io` publish before v1 | Use `cargo install --git` until stable |

---

## Implementation Sequence

### Phase 1: Correctness & Security (must-fix before open-source)

1. `std::Mutex` → `tokio::sync::RwLock` in `crates/server`
2. Bind to `127.0.0.1` by default (server + Vite dev)
3. Config persistence to `~/.octomonitor/config.json`
4. Fix silent demo data fallback — show clear "Server offline" indicator
5. Port conflict detection with clear error message

### Phase 2: Code Quality (first-impression blockers)

6. Split `main.rs` into modules (state, probe, handlers/*)
7. Event-driven WS with initial snapshot on connect; eliminate bootstrap/WS race
8. Add `ts-rs` derive to `crates/core` types; generate TS types
9. Decouple `build_bootstrap()` from `demo_bootstrap()`
10. Split `SettingsView.tsx` into sub-components
11. Extract shared `formatTokens` / `formatCost` to `lib/format.ts`
12. Remove `@vitejs/plugin-legacy` and `terser` dev dep
13. Add React Error Boundary
14. Desktop `Cargo.toml` inherit `workspace.package` metadata
15. Parallel adapter probing with `tokio::join!`

### Phase 3: Frontend Quality

16. Install Tailwind CSS v4; migrate components from `styles.css`
17. Loading skeletons for MonitorView and UsageView
18. CSS transitions on session card state changes
19. Keyboard shortcut system (global `keydown` handler)
20. Desktop notification on `waitingApproval`

### Phase 4: Open-Source Polish

21. `README.md` with screenshot and quickstart
22. `CONTRIBUTING.md` with architecture overview
23. `LICENSE` file (MIT)
24. CI pipeline

---

## Metrics for "Done"

- [ ] `cargo clippy -- -D warnings` passes
- [ ] No `std::Mutex` in any async code path
- [ ] Server binds to `127.0.0.1` by default
- [ ] `main.rs` is under 100 lines
- [ ] Types auto-generated from Rust; CI fails on drift
- [ ] Config survives server restart
- [ ] WS sends initial snapshot on connect; no separate bootstrap fetch in frontend
- [ ] WS only sends subsequent frames when data actually changes
- [ ] Demo data fallback clearly labeled in UI
- [ ] Error boundary catches component crashes
- [ ] `styles.css` removed; all styling via Tailwind
- [ ] Loading skeleton visible on cold start
- [ ] Keyboard shortcuts functional
- [ ] README.md with screenshot exists
