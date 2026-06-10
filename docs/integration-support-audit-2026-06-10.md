# CLI Integration Support Audit (2026-06-10)

Scope: revisit the CLI Hooks list shown in Vibe Island settings and decide which tools OctoMonitor can already count, display, and operate on, and which tools are feasible expansion targets.

Current product boundary: OctoMonitor is still local-first and read-only by default. "Operate" currently means safe local affordances such as inspect details, show/copy a resume command, open a desktop deep link, or report diagnostics. It does not approve prompts, kill turns, mutate tool config, or reuse private OAuth tokens unless a later feature explicitly introduces a user-confirmed operation model.

This file is an audit of current support and the Vibe Island screenshot set. The broader target-state plan from the follow-up market research lives in [Agent Control Plane Roadmap](./plan/2026-06-10-agent-control-plane-roadmap.md).

## Third-Pass Market Expansion

The broader research pass moves the target beyond the screenshot tools. The recommended target is a local-first CLI agent control plane: passive monitoring by default, live state through official hooks/API/RPC where available, operations exposed through per-adapter capability flags, and reliable jump/open/resume flows.

Target additions:

- Promote as high-priority monitoring targets: Gemini CLI, Pi Agent, OpenCode / opencode, GitHub Copilot CLI, OpenHands CLI, Kiro CLI, Kimi Code, Goose, and Qwen Code after path/schema fixtures.
- Treat CodeBuddy as managed-first and fixture-gated: official headless JSON/stream-json and hooks are valuable, but passive store parsing should wait for locked package-source or real-machine fixtures.
- Treat Continue `cn`, Cline CLI, and Cursor Agent as candidate/experimental until session schema fixtures are locked.
- Treat Amazon Q Developer CLI as legacy now that AWS documents that Q CLI has become Kiro CLI.
- Keep WorkBuddy as detection-only until a real WorkBuddy install confirms CodeBuddy-like session files.
- Treat Aider as a workspace-local helper rather than a global monitoring target because it has repo-local markdown history but no durable global session model.
- Keep Amp and Windsurf/Cascade out of passive monitoring until official local session APIs or unencrypted transcript stores exist.
- Keep Roo Code out of scope until there is official CLI or stable local session evidence.
- Treat Codex as the operation-layer reference because app-server exposes thread, turn, approval, and interrupt primitives.
- Treat OpenCode, OpenClaw, CodeBuddy, and managed Pi/Cline/OpenHands as follow-on operation bridges where official APIs or managed process ownership exist.
- Keep Jump Links Lite as the default jump product; precise existing-terminal focus is only a target for OctoMonitor-managed terminal sessions.

## Vibe Island Boundary

The Vibe Island Integrations page combines three product surfaces:

- CLI hook toggles that auto-configure per-CLI hooks on launch.
- Terminal title / jump rules that route users back to Ghostty, Warp, or other terminal tabs.
- Per-CLI enable/disable controls.

OctoMonitor should not clone that surface wholesale. Its core value is cross-tool counting and display, not terminal focus control or automatic hook mutation. A safe operation boundary remains:

- Supported: inspect details, copy/show resume commands, Codex desktop deep links, and read-only diagnostics.
- Acceptable if explicit: per-CLI source enable/disable, user-confirmed hook install/uninstall with preview and backup, and lightweight "open/resume" links.
- Still not acceptable as default behavior: silent hook config writes, broad prompt approval, cross-tool kill-turn controls, global terminal-title ownership, and brittle terminal-tab focus hacks.

## Operation Layer Cost / Return

| Capability | Estimated cost | Return | Risk | Recommendation |
|------------|----------------|--------|------|----------------|
| Per-CLI enable/disable | S-M, 2-4 days | High: reduces noise, lets users demote experimental/candidate sources, and matches the screenshot's mental model. | Low: local preferences plus probe/source filtering. | Add to plan. This is the safest Vibe Island-like control. |
| Claude/Hermes resume commands | S, 1-2 days | Medium-high: immediate utility and fits existing Inspect/CopyButton patterns. | Low-medium: depends on reliable session id mapping. | Add to P1. Keep as advisory copy/open command, not process control. |
| Explicit Hook Manager | M, 4-8 days for Claude/Codex/Gemini first; +1-2 days per extra CLI | High: improves live data quality and removes manual setup friction. | Medium: must merge existing JSON/TOML safely, preserve user config, and provide uninstall. | Add to plan as opt-in only: detect, preview diff, backup, install, verify, uninstall. No silent Doctor writes. |
| Jump Links Lite | M, 3-5 days for macOS desktop first | Medium: "get me back to work" is useful, especially from notifications. | Medium: terminal support differs; should open/resume or create a new tab, not promise exact old-tab focus. | Add to plan after Hook Manager. Start with Codex deep link, resume-command launch, and Warp URI/new-tab style links. |
| Precise terminal tab jump / title ownership | L-XL, 2-4 weeks for macOS only plus ongoing per-terminal maintenance | Medium for heavy terminal users, low for web/server users. | High: requires terminal-specific metadata, accessibility/automation permissions, and fragile title/window matching. | Research spike only. Do not commit to full parity until Jump Links Lite proves demand. |
| Approve / ask / kill turn | L-XL, multi-week and tool-specific | Medium: useful when an agent is stuck, but not central to monitoring. | High: permission and safety boundary changes; APIs vary by tool. | Do not add broadly. Consider a Codex-only `turn/interrupt` experiment after an explicit operation permission model exists. |

Cost conclusion: the acceptable investment is a phased "Operation Pack", not a Vibe Island clone. Per-CLI switches, resume commands, and an explicit Hook Manager have good return for manageable cost. Precise terminal focus and cross-tool approval/kill controls are too brittle or risky for the near-term roadmap.

## Summary Matrix

Candidate rows incorporate the source-level second pass at the end of this document. Where passive local stores are source-verified, the roadmap now treats passive scan as the primary expansion path; hooks and managed process launch are secondary surfaces for live state or future operation.

| Tool | Current level | Count/display today | Operate today | Expansion verdict |
|------|---------------|---------------------|---------------|-------------------|
| Claude Code | Monitored | Yes: local transcript scan plus statusline/hook ingest paths feed sessions, tokens, cost, state, workspace, and details. | Copy-resume is available when a session id is present; no mutating operation bridge is enabled. | Keep current adapter. Extend only through official statusline/hooks/OTel-style surfaces. |
| Codex | Monitored | Yes: local session scan, hook ingest, Codex event parser, event timeline, token/state/project display. | Resume command and desktop deep link when thread id is available. | Update docs/config wording to canonical `[features].hooks`; app-server can unlock richer operations later. |
| OpenClaw | Monitored | Yes: Gateway/session-store probing, usage, health/source status, and session records. | Read-only monitor/status surfaces. | Keep current adapter; use Gateway/schema sources before editing any OpenClaw config. |
| Hermes | Experimental | Yes: Hermes local state/profile scan and Gateway status are displayed, but adapter remains experimental. | Copy-resume is available when profile/session metadata is present; no first-class control operations today. | Upstream now exposes richer CLI status/session data; keep experimental label until adapter is hardened against the SQLite/state surfaces. |
| Gemini CLI | Experimental, fixture-gated | Yes when the local JSONL store matches locked fixtures. Usage/model/cwd can be counted; hook ingest is opt-in for live approval metadata. | None. | Keep OAuth/provider files outside scan scope. Hooks remain the only reliable source for transient "awaiting approval"; do not harvest OAuth tokens. |
| Cursor Agent | Experimental opt-in | Yes only when `OCTOMONITOR_CURSOR_PRIVATE_STORE=1` is set. It displays sessions/model/activity; usage must show N/A. | None. | Format is undocumented & beta. Keep disabled by default and treat usage as N/A because upstream stores no token usage. |
| WorkBuddy / CodeBuddy | CodeBuddy experimental fixture-gated; WorkBuddy detection-only | CodeBuddy can be counted from locked Claude-like JSONL fixtures. WorkBuddy remains detection-only until a real install confirms the same layout. | None. | CodeBuddy is high ROI; WorkBuddy must not be promoted past detection-only without real-machine evidence. |
| Pi Agent | Experimental, fixture-gated | Yes when the local JSONL store matches locked fixtures; usage/cost/provider/model can be counted. | None. | Use file scan, not RPC. Pi RPC operations remain out of scope because Pi has no built-in permission sandbox. |

## Existing Supported Adapters

### Claude Code

OctoMonitor's existing Claude adapter still fits the current official surface. Claude Code documents hooks as lifecycle events with JSON on stdin, including session, tool, permission, notification, and stop-style events. That is compatible with OctoMonitor's current read-only ingest approach.

Decision:
- Keep Claude as "Monitored".
- Do not add write/config mutation behavior in Environment & Doctor.
- Add `claude --resume <session_id>` support as a small, explicit resume-command improvement once the adapter can map the right session id reliably.
- Future improvements should prefer official statusline, hook, or telemetry data over derived transcript inference.

Sources:
- Official hooks reference: <https://code.claude.com/docs/en/hooks>

### Codex

Codex support remains valid, but one doc/config detail changed: official Codex docs now say hooks are enabled by default and `[features].hooks` is the canonical key; `codex_hooks` remains only as a deprecated alias. Codex app-server also exposes structured thread lifecycle APIs such as `thread/list`, `thread/read`, `thread/resume`, `turn/start`, and `turn/interrupt`.

Decision:
- Keep Codex as "Monitored".
- Update OctoMonitor docs to avoid old `codex_hooks` wording.
- Treat app-server as the safest path for future richer Codex operations, but do not expose mutating operations until permissions and local-only constraints are designed.
- Do not add a Vibe Island-style hook toggle. If hook setup is needed, expose a deliberate template/instructions path rather than silent config writes.

Sources:
- Official Codex hooks: <https://developers.openai.com/codex/hooks>
- Official Codex app-server API overview: <https://developers.openai.com/codex/app-server#api-overview>
- GitHub source checked at `openai/codex` HEAD `608b8b1cc6ce91064e1fd12e0810e1772b5e4710`: <https://github.com/openai/codex>

### OpenClaw

OpenClaw remains one of the monitored tools in this repo. Current docs emphasize Gateway configuration, live schema lookup, sessions, plugins, MCP, and source-of-truth Gateway surfaces. OctoMonitor should continue using Gateway/session-store surfaces and avoid config edits from Doctor.

Decision:
- Keep OpenClaw as "Monitored".
- Continue making Gateway/source status visible.
- For future config-aware operations, use `openclaw config schema` / schema lookup rather than hard-coded JSON edits.

Sources:
- Official configuration reference: <https://docs.openclaw.ai/gateway/configuration-reference>
- Session source docs in GitHub: <https://github.com/openclaw/openclaw/blob/main/docs/concepts/session.md>
- GitHub source checked at `openclaw/openclaw` HEAD `6c045c5ca3a15678174b093d5e93ea74867a851d`: <https://github.com/openclaw/openclaw>

### Hermes

Hermes is more mature upstream than this repo's "experimental" label might suggest. The official CLI docs show resume commands, session storage in `~/.hermes/state.db`, status bar token/cost/duration display, `/usage`, `/status`, background sessions, profiles, and Gateway-oriented operation.

Decision:
- Keep Hermes visible but "Experimental".
- Prefer officially documented session/status surfaces over older assumptions.
- Do not promote to fully monitored until adapter tests cover the current SQLite/state/profile model.
- Add `hermes --resume <session_id>` as a near-term advisory operation because the official CLI supports it and the adapter already has session ids.

Sources:
- Official CLI docs: <https://hermes-agent.nousresearch.com/docs/user-guide/cli>
- Official docs home: <https://hermes-agent.nousresearch.com/docs/>
- GitHub source checked at `NousResearch/hermes-agent` HEAD `7df3aa34b17819c790098c391a88ea0ab0827f4d`: <https://github.com/NousResearch/hermes-agent>

## Candidate Integrations

### Gemini CLI

Gemini CLI is a high-ROI candidate because it now has two useful surfaces: default local chat recordings and official hooks. Source verification found `~/.gemini/tmp/<slug>/chats/session-*.jsonl` with full usage/model/cwd data. Official hooks remain valuable for live transient state, especially tool-permission notifications, because persisted chat records mostly capture terminal states.

Recommended adapter shape:
- First phase: passive JSONL scan of `~/.gemini/tmp/<slug>/chats/session-*.jsonl`, resolving project roots through `projects.json` or `.project_root` markers and honoring `$set` / `$rewindTo` lines.
- Second phase: optional hook ingest through the explicit Hook Manager for live state such as awaiting approval.
- Third phase: `~/.gemini` local telemetry file watcher/parser where the user explicitly configured telemetry.
- Count/display: session id, cwd, model, token usage, tool events, and active/done state where available.
- Operation: diagnostic only at first; do not auto-edit `~/.gemini/settings.json` outside the explicit Hook Manager.
- Security rule: do not read or reuse Gemini CLI OAuth credentials.

Sources:
- Hooks reference: <https://github.com/google-gemini/gemini-cli/blob/main/docs/hooks/reference.md>
- Telemetry docs: <https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/telemetry.md>
- `/stats model` usage docs: <https://github.com/google-gemini/gemini-cli/blob/main/docs/get-started/index.md>
- GitHub source checked at `google-gemini/gemini-cli` HEAD `3a13b8eeb65dc9177c07af814aa7411744ab0b1b`: <https://github.com/google-gemini/gemini-cli>

### Cursor Agent

Cursor Agent can be counted/displayed passively, but not as a usage-complete adapter. Official docs describe `agent -p`, `--output-format json|stream-json`, session ids in structured output, and `agent acp` for Agent Client Protocol over stdio. The second-pass reverse-engineering and third-party verification found the CLI store at `~/.cursor/chats/<hash>/<uuid>/store.db` with SQLite plus hex-encoded JSON, which is enough for sessions/model/content/activity but not token usage.

Important constraint: Cursor usage must display as N/A because token usage is not stored in the local CLI store and is not emitted by `stream-json`. The public `cursor/cursor` GitHub repository does not expose the CLI implementation source, so this remains a defensive parser against an undocumented beta format. Community reports also indicate stream-json has edge cases such as reconnecting events and partially documented fields.

Recommended adapter shape:
- First phase: passive read-only SQLite+hex parser for `~/.cursor/chats/**/store.db`.
- Second phase: support OctoMonitor-launched `agent -p --output-format stream-json` sessions only as an optional managed-session path.
- Third phase: optional ACP bridge for managed sessions if it adds enough control value.
- UI rule: count/display sessions, model, activity, and transcript metadata; show token/cost usage as N/A rather than inferred.

Sources:
- CLI overview: <https://cursor.com/docs/cli/overview>
- Headless mode: <https://cursor.com/docs/cli/headless>
- Output format: <https://cursor.com/docs/cli/reference/output-format>
- ACP: <https://cursor.com/docs/cli/acp>
- Public GitHub repo checked at `cursor/cursor` HEAD `654b1b4775ca67aef473bd31a14c8c04a1abde2d`: <https://github.com/cursor/cursor>

### WorkBuddy / CodeBuddy

The most concrete public documentation is CodeBuddy. It documents `codebuddy -p`, `--output-format json|stream-json`, `--include-partial-messages`, `--resume`, `--continue`, `--worktree`, background sessions, plugin hooks, and the `~/.codebuddy` / `CODEBUDDY_CONFIG_DIR` configuration model. Source verification confirms CodeBuddy uses Claude-Code-like JSONL under `~/.codebuddy/projects/**/*.jsonl`, plus PID/session liveness files. WorkBuddy appears to be the same engine under `~/.workbuddy` / `WORKBUDDY_CONFIG_DIR`, but that exact on-disk layout still needs validation on a real WorkBuddy install.

Recommended adapter shape:
- First phase: passive CodeBuddy JSONL scan, modeled after the Claude transcript adapter, plus `~/.codebuddy/sessions/<pid>.json` liveness.
- Second phase: validate WorkBuddy's `~/.workbuddy` layout and enable the same adapter when the schema matches.
- Third phase: hook/plugin-based ingest for user-managed sessions.
- Fourth phase: parse `codebuddy -p --output-format stream-json` only if managed launch becomes useful.

Sources:
- CLI reference: <https://www.codebuddy.ai/docs/cli/cli-reference>
- Headless mode: <https://www.codebuddy.ai/docs/cli/headless>
- Plugin hooks/reference: <https://www.codebuddy.ai/docs/cli/plugins-reference>
- Installation/config directory: <https://www.codebuddy.ai/docs/cli/installation>

### Pi Agent

Pi is open source and has a passive local store that fits OctoMonitor's current adapter model better than RPC-first integration. Source verification found append-only JSONL sessions under `~/.pi/agent/sessions/--<cwd>--/<ts>_<id>.jsonl`, with provider/model, usage, cost, tool calls, and branch-tree metadata. Its RPC docs remain relevant for future operation features, but the repo also warns that Pi has no built-in permission sandbox by default, so any operation bridge must be gated separately.

Recommended adapter shape:
- First phase: passive scan of `~/.pi/agent/sessions/**.jsonl`, honoring `--session-dir`, `PI_CODING_AGENT_SESSION_DIR`, and `settings.json:sessionDir`.
- Count/display: session name, provider/model, prompt/response/tool events, state, tree/session metadata, token usage, and cost where present.
- Operation: none at first. RPC (`pi --mode rpc`) belongs to a later explicit operation model because Pi does not include a built-in process/filesystem/network sandbox.

Sources:
- Official site: <https://pi.dev/>
- RPC docs: <https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md>
- GitHub source checked at `earendil-works/pi` HEAD `9ccfcd7cfcacdf593c0b24929d1d847e6cdf6711`: <https://github.com/earendil-works/pi>

## Vibe Island Feature Gap

| Vibe Island feature | OctoMonitor stance | Reason |
|---------------------|--------------------|--------|
| CLI hook auto-config toggles | Implement as explicit Hook Manager | Silent writes still conflict with read-only Doctor, but a preview/backup/install/uninstall flow has acceptable cost and high return. |
| Disable Claude native terminal title | Research only | Title ownership only matters for precise terminal return. It should not become a global app setting unless a jump feature proves worth it. |
| Custom jump rules / terminal URL schemes | Implement "Jump Links Lite"; research precise tab jump | Warp supports URI-based open/new-tab flows, iTerm2 exposes tab/session automation APIs, and Ghostty has AppleScript/config surfaces, but exact existing-tab focus is terminal-specific and fragile. |
| Approve / ask / kill turn | Defer broad support; consider Codex-only interrupt experiment | OctoMonitor may notify pending approval. Actual approval/interrupt changes the safety boundary and needs explicit user permission plus tool-specific APIs. |

Terminal integration source notes:
- Warp URI scheme: <https://docs.warp.dev/terminal/more-features/uri-scheme/>
- Warp Tab Config URL support: <https://docs.warp.dev/terminal/windows/tab-configs/>
- iTerm2 Python API tab/session automation: <https://iterm2.com/python-api/tab.html>
- iTerm2 scripting overview: <https://iterm2.com/documentation-scripting.html>
- Ghostty configuration reference: <https://ghostty.org/docs/config/reference>
- Ghostty custom URL scheme limitation discussion: <https://github.com/ghostty-org/ghostty/discussions/9999>

## Screenshot-Set Roadmap Order

This narrow order applies only to the original Vibe Island screenshot set and the cost-aware discussion in this audit. For the target-state "best-in-market" control-plane plan, use [Agent Control Plane Roadmap](./plan/2026-06-10-agent-control-plane-roadmap.md).

1. P1, 1-2 days: add Claude and Hermes resume commands; harden Hermes tests around SQLite/profile state; update stale Codex hook docs to `[features].hooks`.
2. P1.5, 2-4 days: add per-CLI source enable/disable preferences. This gates probes and display without mutating external tools.
3. P2, adapter expansion: implement passive local scans in the Deep-Dive order: CodeBuddy/WorkBuddy first, Gemini second, Pi third, Cursor fourth. Cursor is display/count only with token/cost usage shown as N/A.
4. P2.5, 4-8 days: build an explicit Hook Manager for Claude/Codex/Gemini first, then extend to CodeBuddy/WorkBuddy only after their passive adapters land. Required properties: detect current config, preview exact diff, create backup, install idempotently, verify ingest, and uninstall cleanly.
5. P3, 3-5 days: prototype Jump Links Lite on macOS desktop. Start with resume/open-new-tab flows, not exact existing-tab focus. Promote only if the prototype works in at least Warp plus one of iTerm2/Terminal/Ghostty without fragile accessibility requirements.
6. P4 gated experiment: consider Codex-only interrupt through app-server after a local operation permission model exists. Do not implement broad approve/kill controls across tools.
7. P5 research only: precise terminal-title ownership and exact terminal-tab focus. Do not commit it to product delivery unless Jump Links Lite proves strong demand and the implementation can avoid brittle accessibility automation.

## Product Changes Made From This Audit

- Environment & Doctor now distinguishes `monitored`, `experimental`, and `candidate` support levels.
- Fixture-gated adapters may appear as experimental monitored-lite sources only when their locked parsers emit runs.
- Detection-only/watchlist tools remain visible for planning/source controls but are not treated as stable monitored data sources.
- README support matrices now state current count/display/operation status for every tool in the screenshot.

---

## Deep-Dive Verification (2026-06-10, second pass)

This section records the source-level pass that corrected the earlier conservative "operate the launch path / hooks-only" framing for the candidate tools. Official docs plus GitHub/npm-bundle source confirm that **all four candidates persist local session data on disk that fits OctoMonitor's existing passive-scan model** (the same model the Claude/Codex/OpenClaw adapters already use). The earlier framing undercounted feasibility because it focused on OctoMonitor-launched/stream-json sessions rather than the on-disk stores. Corrected findings below.

### Revised feasibility matrix (passive local-file scan)

| Tool | Local store (verified) | Format | Tokens/model/cwd on disk? | Passive-scan verdict | Effort |
|------|------------------------|--------|---------------------------|----------------------|--------|
| Gemini CLI | `~/.gemini/tmp/<slug>/chats/session-*.jsonl` (+ legacy `*.json`) | JSONL (append, mixed line kinds) | ✅ all (input/output/cached/thoughts/tool/total, model, cwd via `directories[]`) | **High — near-isomorphic to Claude/Codex scan** | Medium |
| CodeBuddy / WorkBuddy | `~/.codebuddy/projects/<proj>/<sessionId>.jsonl` + `~/.codebuddy/history.jsonl` + `~/.codebuddy/sessions/<pid>.json`; WorkBuddy mirrors under `~/.workbuddy/` | JSONL (Claude-Code-identical) | ✅ all (`message.usage`, `message.model`, `cwd`); PID files give live-state | **Highest — CodeBuddy is a Claude Code reskin; fork the Claude adapter** | Low |
| Pi Agent | `~/.pi/agent/sessions/--<cwd>--/<ts>_<id>.jsonl` | JSONL (tree, `id`/`parentId`) | ✅ all (`usage` incl. cache+cost, `provider`, `model`) | **High — like Codex/Claude scan, plus a branch tree** | Medium |
| Cursor Agent | `~/.cursor/chats/<hash>/<uuid>/store.db` | **SQLite + hex-encoded JSON** (2 tables: `meta`, `blobs`) | ❌ **no token usage anywhere** (confirmed product gap); session name/model/content yes | **Partial — sessions/content/model/activity OK, usage = N/A** | High (new SQLite+hex parser; format undocumented & beta) |

### Per-tool corrections

**Gemini CLI** — Default-on, no user config required (decisive advantage over its telemetry path). Two implementation gotchas vs. the old doc: (1) the project temp dir is now keyed by a **human-readable slug**, not `sha256(cwd)` — resolve cwd via `~/.gemini/projects.json` or each dir's `.project_root` marker (docs still say `<project_hash>`; source `storage.ts` + `projectRegistry.ts` is authoritative). (2) JSONL lines include `$set` (metadata/messages overwrite) and `$rewindTo` (truncation) control lines that the parser must honor. Hooks (`SessionStart`/`Notification(ToolPermission)`/`BeforeTool`/`AfterTool`, common fields `session_id`/`transcript_path`/`cwd`) remain the only reliable source for the transient "awaiting approval" state, since tool-call status is persisted only at terminal states. Source: `packages/core/src/{config/storage.ts,config/projectRegistry.ts,services/chatRecordingService.ts,hooks/types.ts}` @ `3a13b8e`.

**CodeBuddy / WorkBuddy** — Source (npm `@tencent-ai/codebuddy-code@2.105.0`) confirms the CLI is a Tencent reskin of Claude Code: identical `~/.codebuddy` layout, JSONL transcript schema, hook fields, env-var naming, and SDK output schema. `getHomeDir()` honors `CODEBUDDY_CONFIG_DIR` (default `~/.codebuddy`); WorkBuddy is the **same engine** under `~/.workbuddy` / `WORKBUDDY_CONFIG_DIR` (`isWorkBuddyProduct()` switch confirmed in bundle). **Best ROI: fork the Claude adapter, swap the dir prefix, recalibrate the project-id sanitizer, and add the `~/.codebuddy/sessions/<pid>.json` liveness probe.** Account-level quota is server-side only (no local "remaining" figure) — derive consumed usage only. ⚠️ Still to confirm on a real machine: that WorkBuddy actually writes `~/.workbuddy/projects/**/*.jsonl` with the same line schema (strong inference, not observed), and WorkBuddy's headless CLI entrypoint/command name.

**Pi Agent** — Open source, fully source-verified @ `9ccfcd7`. Sessions are append-only JSONL bucketed by cwd; first line is a `type:"session"` header with the working dir, subsequent entries carry `usage` (input/output/cacheRead/cacheWrite + cost breakdown), `provider`, `model`, tool calls, and a `id`/`parentId` branch tree. Honor redirects: `--session-dir`, `PI_CODING_AGENT_SESSION_DIR`, and `settings.json:sessionDir`. State is inferred from last-entry type + mtime (same approach as the Codex adapter). RPC (`pi --mode rpc`) is a stdin/stdout control channel — out of scope for passive monitoring; relevant only if an "operate" feature is added later, where Pi's **no-built-in-sandbox** stance (README) puts all risk on the caller.

**Cursor Agent** — CLI store is `~/.cursor/chats/<hash>/<uuid>/store.db` (SQLite; `meta` + `blobs` tables, values hex-encoded JSON) — distinct from the Cursor **IDE** store (`state.vscdb`). This is what `agent ls` / `--resume` read, and the existing third-party `agent-sessions` tool already passively indexes it (alongside Codex/Claude/Gemini/OpenClaw/Hermes — nearly OctoMonitor's exact set), proving the model works. But: **no token/usage is stored anywhere** (a confirmed, open upstream feature request) and `stream-json` also omits tokens, so Cursor can be counted/displayed for sessions/model/activity but must show usage as **N/A**. Format is undocumented and beta → defensive parsing required. Lowest ROI for a usage-centric monitor.

### Revised implementation order

This adapter-only order is folded into Roadmap P2 above.

1. **CodeBuddy / WorkBuddy** — lowest effort (fork Claude adapter), full usage, immediate value.
2. **Gemini CLI** — JSONL scan with slug-dir + `$set`/`$rewindTo` handling; default-on, full usage.
3. **Pi Agent** — JSONL tree scan; full usage incl. cost; honor session-dir redirects.
4. **Cursor Agent** — SQLite+hex parser; sessions/model/activity only, usage N/A; defer until 1–3 land.

### Supported-tool follow-ups (no architecture change)

- **Codex**: docs/config wording should standardize on canonical `[features].hooks` and drop the deprecated `codex_hooks` alias.
- **Hermes**: upstream is now SQLite-backed (`~/.hermes/state.db`); keep the "experimental" label until the adapter is tested against that store, then consider promotion to `monitored`.
- **Claude / OpenClaw**: no change; keep preferring official statusline/hooks/Gateway surfaces over derived inference.

### Sources added this pass

- Gemini CLI source @ `3a13b8eeb65dc9177c07af814aa7411744ab0b1b`: <https://github.com/google-gemini/gemini-cli>
- CodeBuddy CLI dir layout/hooks: <https://www.codebuddy.ai/docs/cli/codebuddy-dir>, <https://www.codebuddy.ai/docs/cli/hooks>; npm `@tencent-ai/codebuddy-code@2.105.0`; WorkBuddy same-engine: <https://docs.evolink.ai/en/integration-guide/codebuddy-workbuddy>
- Pi source @ `9ccfcd7cfcacdf593c0b24929d1d847e6cdf6711`: `packages/coding-agent/src/core/session-manager.ts`, `packages/coding-agent/docs/{session-format.md,rpc.md}`
- Cursor CLI store reverse-engineering + same-class tool: <https://cursor.com/docs/cli/using>, <https://jazzyalex.github.io/agent-sessions/>, <https://dev.to/vineethnkrishnan/building-agent-sessions-a-universal-session-manager-for-the-ai-cli-era-2i04>
