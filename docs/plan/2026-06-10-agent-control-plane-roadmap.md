# Agent Control Plane Roadmap (2026-06-10 Research)

Source: synthesis of the 2026-06-10 deep research pass on CLI coding-agent integrations, operation surfaces, and jump/terminal integration. This document is a target-state plan, not a statement of what OctoMonitor supports today.

## Goal

Raise OctoMonitor from a local monitor into a local-first CLI agent control plane:

- See state: sessions, workspace, model, token/cost, tool events, approval state, errors, and last activity.
- Count usage: aggregate by tool, workspace, model, time, token, cost, and session.
- Operate safely: resume, open, install/uninstall hooks, interrupt/cancel where official APIs exist, and respond to approvals only when the native API exposes the exact request.
- Jump reliably: open workspace, copy command, resume in a new terminal tab, open native app/deep link, and focus existing terminals only for OctoMonitor-managed sessions.

The product should not become a cross-tool remote-control layer that bypasses each CLI's safety model.

## Non-Negotiable Boundaries

- No silent hook/config writes.
- No broad prompt approval or "approve all tools" control.
- No cross-tool kill-turn abstraction.
- No global terminal-title ownership.
- No brittle old-tab focus promise for sessions OctoMonitor did not manage.
- No stdin injection into arbitrary terminals.
- No reading OAuth tokens, API keys, `.env`, or provider secrets.
- No inferred cost when the data is absent; show `N/A` or `estimated`.
- No prompt/transcript body in dashboard summaries by default; full transcript details require an explicit local-only user action.
- No scanning files whose path or name indicates credentials, auth, tokens, provider settings, or environment secrets.

## Target Layers

| Layer | Purpose | Data/source preference |
|------|---------|------------------------|
| Monitoring Core | Normalize sessions, usage, details, and timelines. | Official APIs first, then stable local stores, then defensive passive scans. |
| Live State Layer | Distinguish running, idle, waiting for input, waiting for approval, completed, error, and cancelled. | API/SSE/RPC, official hooks, process liveness, then passive inference. |
| Operation Layer | Expose safe actions per tool. | Capability flags from each adapter, never hard-coded by tool name alone. |
| Jump Layer | Return users to the right work context. | Copy command and new terminal tab by default; managed focus only when OctoMonitor captured terminal identity. |
| Safety Layer | Gate, confirm, audit, and roll back mutations. | Per-action confirmation, permission grants, hook backups, and local audit logs. |

## Target Support Matrix

| Tool | Repo status today | Target level | Primary data source | Operation target | Confidence |
|------|-------------------|--------------|---------------------|------------------|------------|
| Claude Code | Monitored | Monitored | `~/.claude/projects/**/*.jsonl`, hooks, statusline | Resume/open/copy; observe approvals; no default approve/deny | High |
| Codex | Monitored | Monitored + Operations Reference | `$CODEX_HOME/sessions/**/rollout-*.jsonl`, `state_5.sqlite`, app-server | Thread resume/start, turn interrupt, approval review through app-server | High |
| OpenClaw | Monitored | Monitored + Gateway Ops | `~/.openclaw/agents/*/sessions`, Gateway/ACP | Prompt/cancel/resume/close with auth/scopes | High |
| Hermes | Experimental | Monitored | `~/.hermes/state.db` SQLite WAL | Resume/open first; Gateway/ACP experimental | High |
| Gemini CLI | Candidate | Monitored | `~/.gemini/tmp/<project>/chats/session-*.jsonl`, hooks | Resume/open/copy; hooks for live state | High |
| CodeBuddy | Candidate | Monitored + Managed Worker Ops after fixtures | Official hooks expose `transcript_path`; `~/.codebuddy/projects/**/*.jsonl`, headless JSON/stream-json statistics, worker registry/liveness fixtures | Resume/continue, passive scanner, managed stream ingest, Hook Manager, verified worker logs/kill only | High |
| WorkBuddy | Candidate | Candidate until verified | `~/.workbuddy` / `WORKBUDDY_CONFIG_DIR` detection only until CLI/session schema is verified | No passive parsing or mutating ops until real-machine validation | Low |
| Pi Agent | Candidate | Monitored + Experimental RPC Ops | `~/.pi/agent/sessions/**/*.jsonl` | Resume/fork; managed RPC follow-up only | High |
| Cursor Agent | Candidate | Experimental | Official CLI `agent ls/resume`; optional `~/.cursor/chats/**/store.db` | Resume/open/headless managed only; usage `N/A` | Medium/Low |
| OpenCode / opencode | Not detected | Monitored + Operations | SQLite `opencode.db` / official CLI `session list/stats/export` / server API | Abort/respond/message with server auth; no arbitrary TUI control | High |
| Qwen Code | Not detected | Monitored + Managed Sidecar Ops after fixtures | Runtime-detected `~/.qwen/projects/<sanitized-cwd>/chats/*.jsonl`, sidecar JSON event stream, hooks | Resume/open/copy; sidecar submit/confirmation only for managed sessions | High/Medium |
| OpenHands CLI | Not detected | Monitored | `~/.openhands/conversations/*/conversation.json`, SDK persistence/events, ACP/REST candidates | Resume/open; managed ACP/REST later; never default `--always-approve` | High |
| Cline CLI | Not detected | Fixture-gated Monitored metadata + Experimental Hub Ops | `~/.cline/data/sessions` SQLite metadata, managed JSON output, hooks/Hub/ACP fixtures | Resume/managed Hub only; force `--auto-approve false` unless explicit advanced opt-in | Medium/High |
| Aider | Not detected | Experimental workspace-local read-only | `.aider.chat.history.md` per workspace only | Workspace-local restore/open/copy only | Medium |
| Kiro CLI | Not detected | CLI/custom-storage Monitored candidate; DB scanner fixture-gated | Official session commands, `KIRO_HOME`, custom storage script JSON; DB path/schema fixture only | `kiro-cli chat --resume-id`, list/resume/delete read-only, managed custom-storage capture, hook ingest | Medium |
| Amazon Q Developer CLI | Not detected | Legacy Candidate | Q CLI has become Kiro CLI; keep legacy detection only | `q chat --resume` only if present; prompt migration to Kiro | Medium |
| GitHub Copilot CLI | Not detected | Monitored + ACP Candidate | `~/.copilot/session-state/` plus `~/.copilot/session-store.db` Chronicle index | Launch/resume/open; ACP/VS Code managed operations after fixtures | High |
| Continue `cn` | Not detected | Monitored-lite read-only | `~/.continue/sessions/*.json` or `CONTINUE_GLOBAL_DIR/sessions/*.json`, permissions/logs diagnostics | `cn --resume`; permissions health only; no Hook Manager | High |
| Kimi Code | Not detected | Monitored | `$KIMI_CODE_HOME/sessions/`, `session_index.jsonl`, `state.json`, `agents/*/wire.jsonl`, hooks, ACP candidate | `kimi --continue` / `--session`, hook ingest, ACP later | High |
| Goose | Not detected | Monitored | `~/.local/share/goose/sessions/sessions.db`, CLI list/export/resume, managed `stream-json` | Resume/list/export; managed run later | High |
| Amp | Not detected | Not suitable for passive monitoring | Local settings only; thread history appears cloud/server-oriented until proven otherwise | Launch/open only after official local store evidence | Low/Medium |
| Windsurf / Cascade | Not detected | Not suitable for passive monitoring | App detector only; avoid encrypted or reverse-engineered Cascade stores | Open app/workspace only | Low |
| Codebuff | Not detected | Candidate | CLI/agents are market-relevant but local session store is unverified | Detection only | Low/Medium |
| Roo Code | Not detected | Not suitable yet | No stable official CLI/local session evidence | None | Low |

## Follow-Up Report Reconciliation

The later simplified and full research reports mostly confirm this roadmap, but they change the priority of several edge tools. Accepted updates:

- **GitHub Copilot CLI moves up**: GitHub's Chronicle/session-data docs confirm `~/.copilot/session-state/` and recoverable `~/.copilot/session-store.db`, so Copilot CLI is no longer only a managed-launch candidate.
- **Continue `cn` moves up to Monitored-lite**: official source uses `CONTINUE_GLOBAL_DIR || ~/.continue` and `sessions/<uuid>.json` with workspace, history, and usage fields. Logs remain diagnostics only, and permissions YAML is read-only.
- **Cline confidence increases for metadata**: official CLI docs confirm `--data-dir`, `--hooks-dir`, `--acp`, and `--id`; package artifacts point to SQLite session metadata and Hub commands, but token/cost/tool events still require fixtures.
- **Aider moves down**: `.aider.chat.history.md` is useful per workspace, but it has no global session model, durable session id, or operation surface suitable for the control plane.
- **Amp and Windsurf/Cascade move down**: avoid cloud-synced or encrypted/reverse-engineered stores until official local session APIs exist.
- **OpenCode moves up**: official CLI/API/server and SQLite schema evidence make it a monitored target, but scanners still need read-only WAL and corruption fixtures.
- **OpenHands moves up**: official conversation path and SDK persistence/statistics evidence make it a monitored target; scanner must redact secrets in saved state.
- **Qwen stays in scope with a clearer path**: latest evidence points release targets at `~/.qwen/projects/<sanitized-cwd>/chats/*.jsonl`; sidecar and hooks are strong enough for managed operation experiments.
- **CodeBuddy moves up again, but with release gates**: official hooks expose `transcript_path`, daemon/headless docs confirm managed statistics and worker surfaces, and passive transcript parsing is worth implementing with golden fixtures.
- **Kiro replaces legacy Amazon Q as the target**: AWS now points Q CLI users to Kiro CLI; Kiro has UUID sessions and custom storage scripts, but the DB filename/schema is not a public release contract, so CLI/custom-storage comes first.
- **Kimi Code is added**: official docs expose `$KIMI_CODE_HOME/sessions/`, `session_index.jsonl`, `state.json`, agent `wire.jsonl`, hooks, and ACP candidate surfaces.
- **Goose is added**: official docs confirm v1.10+ SQLite `sessions.db`, list/resume/export, and managed `stream-json`; it is a monitored target after schema fixtures.

## Operation Capability Model

Adapters should expose capabilities, not UI assumptions.

```ts
type OperationCapability =
  | "resume.copyCommand"
  | "resume.launchNewTerminal"
  | "resume.nativeApi"
  | "open.workspace"
  | "open.nativeApp"
  | "open.sessionDeeplink"
  | "turn.start"
  | "turn.steer"
  | "turn.interrupt"
  | "approval.observe"
  | "approval.respond"
  | "process.attach"
  | "process.killOwned"
  | "hook.install"
  | "hook.verify"
  | "hook.uninstall";

interface CapabilityDescriptor {
  id: OperationCapability;
  source: "official-api" | "official-cli" | "official-hook" | "reverse-engineered" | "inferred";
  confidence: "high" | "medium" | "low";
  mutatesState: boolean;
  requiresUserConfirmation: boolean;
  requiresManagedProcess: boolean;
  canExposeSecrets: boolean;
  auditLevel: "none" | "metadata" | "full";
  failureMode: "safe" | "may-leave-process-running" | "may-drop-data";
}
```

Recommended capability policy:

- `resume.*`: expose broadly when the tool has official resume IDs or commands.
- `open.*`: expose broadly, with reliability labels.
- `hook.*`: expose only through Hook Manager.
- `turn.interrupt`: Codex, OpenCode, OpenClaw, CodeBuddy workers, and managed Pi/Cline sessions.
- `approval.respond`: Codex first; OpenCode/OpenClaw later only if the exact command/diff/request is shown.
- `turn.start` / `turn.steer`: official API or managed session only.
- `process.killOwned`: only CodeBuddy official worker APIs or OctoMonitor-owned processes.

## Unified Session Model

The existing run/session model should grow toward a normalized session envelope:

```ts
interface UnifiedSession {
  id: string;
  tool: string;
  sourceId: string;
  title?: string;
  project?: {
    cwd?: string;
    workspaceRoot?: string;
    projectHash?: string;
    repo?: { root?: string; branch?: string; worktree?: string };
  };
  model?: {
    provider?: string;
    name?: string;
    displayName?: string;
    contextWindow?: number;
  };
  lifecycle: {
    status:
      | "running"
      | "idle"
      | "waiting_for_input"
      | "waiting_for_approval"
      | "completed"
      | "error"
      | "cancelled"
      | "unknown";
    statusSource: "api" | "hook" | "process" | "passive" | "inferred";
    startedAt?: string;
    lastActivityAt?: string;
    endedAt?: string;
    error?: string;
  };
  usage?: {
    inputTokens?: number;
    outputTokens?: number;
    cacheReadTokens?: number;
    cacheWriteTokens?: number;
    reasoningTokens?: number;
    toolTokens?: number;
    totalTokens?: number;
    costUsd?: number;
    costKind: "exact" | "estimated" | "not_available";
    source: "transcript" | "api" | "statusline" | "computed" | "unknown";
  };
  counts: {
    messages?: number;
    toolCalls?: number;
    approvals?: number;
    errors?: number;
  };
  capabilities: CapabilityDescriptor[];
  dataSources: DataSourceHealth[];
  jumpTargets: JumpTarget[];
  toolSpecific: Record<string, unknown>;
}
```

## Data Source Health

Each adapter should report source health independently from session data:

```ts
interface DataSourceHealth {
  id: string;
  type: "jsonl" | "sqlite" | "markdown" | "api" | "hook" | "rpc" | "process" | "terminal";
  path?: string;
  apiEndpoint?: string;
  lastSeenAt?: string;
  schemaVersion?: string;
  schemaConfidence: "high" | "medium" | "low" | "unsupported";
  errors: Array<{
    code: string;
    message: string;
    firstSeenAt: string;
    lastSeenAt: string;
  }>;
}
```

Low-confidence sources should not feed usage totals or mutating operations. They may still show metadata with a clear warning.

## Jump Target Model

```ts
interface JumpTarget {
  kind:
    | "copy_command"
    | "new_terminal_tab"
    | "workspace"
    | "native_app"
    | "session_deeplink"
    | "managed_terminal_focus";
  label: string;
  command?: string[];
  cwd?: string;
  url?: string;
  terminal?: {
    provider: "warp" | "iterm2" | "ghostty" | "terminal.app" | "vscode" | "cursor" | "tmux";
    windowId?: string;
    tabId?: string;
    paneId?: string;
    pid?: number;
  };
  reliability: "high" | "medium" | "low";
  requiresConfirmation: boolean;
}
```

Jump tiers:

| Tier | Behavior | Reliability | Scope |
|------|----------|-------------|-------|
| 0 | Copy resume command | High | Every tool with resume command. |
| 1 | Open workspace/app | High | Finder, VS Code, Cursor, native app, browser UI. |
| 2 | Open new terminal tab and run resume | Medium/High | Warp, iTerm2, Ghostty, Terminal.app, VS Code terminal. |
| 3 | Focus existing tab/session | High only for managed sessions | tmux/iTerm2/Ghostty/Terminal sessions launched or registered by OctoMonitor. |

Precise focus must never rely only on terminal title search. A managed jump target needs captured provider identity plus PID/cwd/session validation.

## Hook Manager

Hook Manager is a product feature, not a background doctor repair.

Required flow:

1. Detect current hook/config state read-only.
2. Explain what will run, what will be collected, and what will not be collected.
3. Preview exact JSON/TOML/YAML/file diff.
4. Create a backup and record sha256.
5. Install the smallest managed block or plugin package.
6. Verify via official list/test/doctor or a short controlled session.
7. Audit actor, time, tool version, source file, old hash, new hash, and result.
8. Uninstall only OctoMonitor-managed blocks.
9. Restore from backup when requested.

Target hook support:

| Tool | Strategy | Risk note |
|------|----------|-----------|
| Claude Code | Plugin-style or explicit settings diff; observe-only by default. | PermissionRequest can approve/deny; keep gated. |
| Codex | Use app-server `hooks/list` and config APIs when available; respect trust state and `[features].hooks`. | Project trust and deprecated aliases matter. |
| Gemini CLI | Observe-only hook in settings; use JSONL passive scan as primary. | Do not modify approval mode or auth. |
| Qwen Code | Observe-only hook; never enable input/response mutation by default. | Hooks can modify model inputs/responses. |
| Kimi Code | TOML managed block in Kimi config; observe-only by default. | `credentials/` and exported debug zips are sensitive. |
| Kiro CLI | Agent config hooks or custom storage script verification. | DB schema is not enough; use storage-script JSON fixtures before promotion. |
| CodeBuddy | Plugin hook package. | Validate plugin scope and executable permissions. |
| Hermes | Use `hermes hooks list/test/revoke/doctor`. | Respect Hermes allowlist. |
| OpenCode | Plugin hook plus server API. | Server auth and plugin permissions must be explicit. |
| Cline | Managed hook directory. | Default auto-approve risk must be surfaced. |
| Cursor | Detect only until hook config schema is verified. | Store and hook behavior are lower confidence. |

## Permission Grants

```ts
interface PermissionGrant {
  id: string;
  tool: string;
  scope:
    | "source.read"
    | "hook.install"
    | "operation.resume"
    | "operation.interrupt"
    | "operation.approval.respond"
    | "operation.sendMessage"
    | "operation.killOwnedProcess"
    | "jump.openNewTerminal"
    | "jump.focusManagedTerminal";
  subject:
    | { type: "global" }
    | { type: "workspace"; path: string }
    | { type: "session"; sessionId: string }
    | { type: "process"; pid: number; startTime: string };
  expiresAt?: string;
  createdAt: string;
  createdBy: "user";
  evidence: {
    capabilitySource: string;
    confidence: "high" | "medium" | "low";
  };
}
```

Remote viewers must remain read-only. Mutating operations belong only on the local admin surface.

## Implementation Roadmap

This order is chosen to get the product to the target state without building unsafe generic controls.

0. Evidence lock and fixtures: for each adapter, capture tool version/tag, evidence URL, schema fingerprint, anonymized fixtures, parser golden tests, and a command replay script before promoting support.
1. Foundation: introduce unified session, data source health, capability descriptors, jump targets, permission grants, and audit log primitives.
2. Current adapter hardening: Hermes read-only `state.db` migration, Codex app-server bridge, Claude/Hermes resume command polish, and OpenClaw Gateway operation gates.
3. P0 monitoring expansion: Gemini, Pi, CodeBuddy passive+managed fixtures, opencode, GitHub Copilot CLI Chronicle, OpenHands conversations, Continue `cn` Monitored-lite, Kimi sessions, Goose SQLite, and Qwen sidecar/path-gated scanner. Add Cursor only as Experimental with usage `N/A`.
4. Managed-first / fixture-gated expansion: Kiro CLI/custom-storage first and DB scanner later; Cline metadata SQLite first and Hub ops later; WorkBuddy detection only until real-machine fixtures.
5. Integration settings: per-CLI enable/disable that stops probes, scans, watchers, and hook ingest while retaining historical data.
6. Hook Manager: Claude, Codex, Gemini first; then Qwen, Kimi, CodeBuddy, Kiro, Hermes, OpenCode, Cline. Cursor detect-only until schema confidence improves.
7. Operation layer: Codex first, then Pi RPC and Hermes ACP, then OpenCode/OpenClaw/CodeBuddy, then managed Qwen/Copilot/Cline/OpenHands. Approval response remains gated by exact native API evidence.
8. Jump Links Lite: copy, open workspace/app, resume in new terminal tab across Warp/iTerm2/Ghostty/Terminal.app/VS Code/Cursor.
9. Managed terminal focus: only for OctoMonitor-launched or explicitly registered tmux/iTerm2/Ghostty/Terminal sessions.
10. Low-confidence watchlist: legacy Amazon Q, WorkBuddy real-machine validation, Aider workspace-local helper, Amp, Windsurf/Cascade, Codebuff, Roo/Kilo extension stores.

## UI / UX Requirements

Integrations page:

- Tool rows show installed state, support level, source health, schema confidence, last seen, hook status, and operation capability count.
- Enable/disable controls stop active collection, not just hide display.
- Hook Manager button opens detect/explain/diff/backup/install/verify/uninstall flow.
- Low-confidence parsers show `Experimental parser` and never feed usage totals by default.

Session detail:

- Primary actions are capability-driven: `Open workspace`, `Resume`, `Copy command`, `Open native app`, `Interrupt`, `Respond to approval`, `Attach`, `View logs`, `Kill owned worker`.
- Mutating actions require a confirmation sheet that shows source, confidence, exact target session/process, and failure mode.
- Usage display distinguishes `exact`, `estimated`, and `N/A`.
- State display distinguishes `api`, `hook`, `process`, `passive`, and `inferred`.

Jump UI:

- Prefer `Resume in new tab` over "jump back" for unmanaged sessions.
- Show `Focus managed terminal` only when provider/window/tab/pane or tmux identity was captured and revalidated.

## Verification Gates

Every adapter needs fixtures for:

- Current schema and at least one older schema.
- Corrupt/truncated line or row.
- Long-running active session.
- Tool call.
- Permission or approval event if available.
- Resume ID extraction.
- Usage exact/estimated/not available.
- Disabled source behavior.
- Secret redaction.

Operations need integration tests for:

- Stale session ID.
- Stale PID / PID reuse.
- Missing auth or expired token.
- Remote viewer denied mutation.
- Interrupted process leaves background work running.
- Hook install, verify, uninstall, restore.

## Evidence Pointers

Primary research sources used for this plan include:

- Codex app-server and hooks docs: <https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md>, <https://developers.openai.com/codex/hooks>
- Claude Code CLI/hooks/statusline docs: <https://code.claude.com/docs/en/cli-reference>, <https://docs.anthropic.com/en/docs/claude-code/hooks>, <https://docs.anthropic.com/en/docs/claude-code/statusline>
- Gemini CLI configuration and chat recorder source: <https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md>, <https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/services/chatRecordingService.ts>
- Cursor CLI docs and ACP references: <https://cursor.com/docs/cli/using.md>, <https://cursor.com/docs/cli/reference/parameters.md>, <https://cursor.com/docs/cli/acp>
- CodeBuddy daemon/plugins docs: <https://www.codebuddy.ai/docs/cli/daemon>, <https://www.codebuddy.ai/docs/cli/plugins-reference>
- CodeBuddy headless JSON/statistics docs: <https://www.codebuddy.ai/docs/cli/headless>
- Pi session/RPC docs: <https://pi.dev/docs/latest/session-format>, <https://pi.dev/docs/latest/rpc>
- Hermes session storage and CLI docs: <https://github.com/NousResearch/hermes-agent/blob/main/website/docs/developer-guide/session-storage.md>, <https://hermes-agent.nousresearch.com/docs/reference/cli-commands>
- OpenClaw Gateway/session/ACP docs: <https://github.com/openclaw/openclaw/blob/main/docs/concepts/session.md>, <https://docs.openclaw.ai/gateway>, <https://docs.openclaw.ai/cli/acp>
- OpenCode CLI/server docs: <https://opencode.ai/docs/cli/>, <https://opencode.ai/docs/server/>
- Qwen Code hooks/session recorder docs: <https://github.com/QwenLM/qwen-code/blob/main/docs/users/features/hooks.md>, <https://github.com/QwenLM/qwen-code/blob/main/packages/core/src/services/chatRecordingService.ts>
- OpenHands CLI docs: <https://docs.openhands.dev/openhands/usage/cli/resume>, <https://docs.openhands.dev/openhands/usage/cli/command-reference>
- Kiro CLI session docs and Amazon Q migration notice: <https://kiro.dev/docs/cli/chat/session-management/>, <https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line.html>
- Kimi Code session docs: <https://moonshotai.github.io/kimi-code/en/guides/sessions.html>
- Goose CLI/session migration docs: <https://goose-docs.ai/docs/guides/goose-cli-commands/>
- GitHub Copilot CLI session data / Chronicle docs: <https://docs.github.com/en/copilot/concepts/agents/copilot-cli/chronicle>
- Continue CLI session source: <https://github.com/continuedev/continue/blob/main/extensions/cli/src/session.ts>
- Cline CLI reference: <https://docs.cline.bot/cli/cli-reference>
- Warp/iTerm2/Ghostty/VS Code jump references: <https://docs.warp.dev/terminal/more-features/uri-scheme/>, <https://iterm2.com/3.4/documentation-scripting.html>, <https://ghostty.org/docs/features/applescript>, <https://code.visualstudio.com/docs/configure/command-line>
