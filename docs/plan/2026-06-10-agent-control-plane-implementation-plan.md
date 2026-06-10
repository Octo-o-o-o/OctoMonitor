# Agent Control Plane Implementation Plan (2026-06-10)

Status: proposed implementation plan after the final evidence-lock research pass.

This plan is a development sequence, not a staged release plan. The intended product outcome is one complete upgrade: OctoMonitor becomes a local-first CLI agent control plane with monitoring, live state, safe operations, hook management, and jump links. Phases exist only to reduce engineering risk and force review/self-test checkpoints.

## Product Outcome

After this plan is complete, OctoMonitor should provide:

- A unified local session fact layer across existing and new CLI agents.
- Accurate usage display with `exact`, `estimated`, `partial`, and `N/A` states.
- Per-source enable/disable that actually stops probes, scanners, watchers, and hook ingest.
- Integration health and schema confidence for each data source.
- Hook Manager with detect, explain, preview diff, backup, install, verify, uninstall, and audit.
- Safe operations exposed only through official APIs, RPC, ACP, sidecar channels, or OctoMonitor-managed processes.
- Jump Links Lite: copy command, open workspace, resume in new terminal tab, native deep link where official, and managed focus only for sessions OctoMonitor launched or registered.
- Strict privacy boundaries: no credentials/auth/token/env scanning, no transcript bodies in dashboard summaries by default, no remote viewer mutations.

## Final Scope

### Stable Or Target-Stable Integrations

| Tool | Target | Primary contract | Operation scope |
|------|--------|------------------|-----------------|
| Claude Code | Existing Monitored, hardened | `~/.claude/projects/**/*.jsonl`, hooks, statusline | Resume/open/copy, hook live state |
| Codex | Existing Monitored + reference ops | rollout JSONL, `state_5.sqlite`, app-server, hooks | Thread resume/start, interrupt, approval response, deep link |
| OpenClaw | Existing Monitored + gateway ops | Gateway/session-store/transcript | Gateway/ACP ops with best-effort labels |
| Hermes | Promote to Monitored | read-only `~/.hermes/state.db` | Resume/open first; ACP ops gated |
| Gemini CLI | New Monitored | `~/.gemini/tmp/<project>/chats/session-*.jsonl`, hooks | Resume/open/copy; hooks for live state |
| Pi Agent | New Monitored + managed RPC ops | `~/.pi/agent/sessions/**/*.jsonl` | Resume/fork; managed RPC get_state/follow_up/abort |
| CodeBuddy | New Monitored + managed worker ops | `~/.codebuddy/projects/**/*.jsonl`, hooks, headless JSON/stream-json, worker registry | Resume/continue, passive scanner, verified worker logs/kill |
| opencode | New Monitored + server ops | `opencode db path`, SQLite schema, CLI stats/export, server API | Abort/respond/message only for paired or managed server |
| GitHub Copilot CLI | New Monitored + ACP candidate | `~/.copilot/session-state/`, `session-store.db` | Resume/open; ACP/VS Code managed ops after fixtures |
| OpenHands CLI | New Monitored | `~/.openhands/conversations/*/conversation.json`, SDK persistence | Resume/open; managed ACP/REST later |
| Continue `cn` | New Monitored-lite | `~/.continue/sessions/*.json` or `CONTINUE_GLOBAL_DIR/sessions/*.json` | Resume only; permissions/logs read-only |
| Qwen Code | New Monitored + managed sidecar ops | `~/.qwen/projects/<sanitized-cwd>/chats/*.jsonl`, hooks, sidecar JSON | Resume/open/copy; managed sidecar submit/confirmation |
| Kimi Code | New Monitored | `$KIMI_CODE_HOME/sessions`, `session_index.jsonl`, `state.json`, `wire.jsonl`, hooks | Resume/open/copy; ACP later |
| Goose | New Monitored | `~/.local/share/goose/sessions/sessions.db`, CLI list/export/resume | Resume/list/export; managed run later |

### Fixture-Gated / Detection-Only

| Tool | Decision | Gate |
|------|----------|------|
| Cline CLI | Fixture-gated Monitored metadata + experimental Hub ops | SQLite schema, Hub event fixtures, `--auto-approve false` launch behavior |
| Kiro CLI | CLI/custom-storage candidate; DB scanner later | custom storage JSON, DB path/schema by OS/version, `KIRO_HOME` fixtures |
| Cursor Agent | Experimental display-only | official CLI bridge first; private stores opt-in only when schema fingerprint matches |
| WorkBuddy | Detection-only | real WorkBuddy install confirms CodeBuddy-like schema |
| Amazon Q Developer CLI | Legacy candidate | detect old installs and point to Kiro; no automatic migration |
| Aider | Workspace-local read-only helper | repo-local history only; no global session model |
| Amp | Managed-only candidate | no passive scan until official local thread store exists |
| Windsurf/Cascade | App-only/detection | no encrypted or reverse-engineered passive parser |
| Codebuff / Roo / Kilo extension stores | Watchlist | official local CLI session evidence required |

## Non-Negotiable Engineering Rules

- Do not read credential/auth/token/env/provider files.
- Do not scan transcript body for dashboard rows when metadata/usage is enough.
- Do not write hooks silently.
- Do not auto-approve prompts.
- Do not kill processes not launched or explicitly registered by OctoMonitor.
- Do not inject stdin into arbitrary old terminals.
- Do not promise exact old-tab focus for unmanaged sessions.
- Remote viewer remains read-only.
- Every low-confidence parser must be schema-gated and excluded from usage totals by default.

## Phase 0 - Evidence Lock And Fixtures

Goal: convert research claims into local, repeatable contracts before implementation promotion.

Tasks:

1. Add a fixture directory convention:
   - `fixtures/agents/<tool>/<version>/<case>/`
   - `evidence_lock.json`
   - `schema_fingerprint.json`
   - `golden_sessions.json`
   - `commands.sh`
   - `README.md`
2. Add an evidence-lock schema:
   - tool id
   - version/tag/package integrity
   - source URL
   - source path/function
   - evidence level
   - local command used to generate fixture
   - denied paths observed
3. Create initial fixtures for:
   - CodeBuddy
   - Continue `cn`
   - Cline
   - Qwen
   - Kiro
   - Kimi
   - Goose
   - Cursor
   - opencode
4. Include negative fixtures:
   - missing path
   - corrupt JSONL
   - truncated JSONL
   - locked/corrupt SQLite
   - unknown schema version
   - credential-looking files
5. Add a fixture test runner script or cargo test helper.

Self-review before leaving phase:

- Every new adapter target has at least one positive and one negative fixture.
- No fixture stores secrets.
- Each fixture has a version/source lock.
- Any claim without a fixture is marked `candidate`, `experimental`, or `detection-only`.

Self-test:

- Run Rust tests for fixture parser helpers.
- Run a secret-pattern scan on fixtures.
- Confirm unrecognized fixtures degrade gracefully instead of panicking.

## Phase 1 - Core Model And Source Health

Goal: add common primitives before expanding adapters.

Tasks:

1. Extend core session model toward:
   - tool/source id
   - workspace/cwd/project hash
   - model/provider
   - lifecycle status and status source
   - usage with confidence
   - operation capability descriptors
   - data source health
   - jump targets
   - tool-specific metadata
2. Add `DataSourceHealth`:
   - source type: jsonl/sqlite/markdown/api/hook/rpc/process/terminal
   - path or endpoint
   - last seen
   - schema version/fingerprint
   - confidence
   - parse errors
3. Add `CapabilityDescriptor`:
   - operation id
   - evidence source
   - confidence
   - mutates state
   - requires confirmation
   - managed-only flag
   - failure mode
4. Add usage semantics:
   - exact
   - estimated
   - partial
   - not available
5. Add central deny-read path matcher:
   - credentials
   - auth
   - token
   - provider settings
   - `.env`
   - MCP env secrets
6. Ensure remote routers do not expose local-only operations.

Self-review:

- Model changes do not break current Claude/Codex/OpenClaw/Hermes display.
- Usage `N/A` cannot be confused with zero.
- Source health is separate from session state.
- Capability flags drive UI, not hard-coded tool names.

Self-test:

- Existing adapter tests.
- Web build/test.
- Remote router route audit.

## Phase 2 - Existing Adapter Hardening

Goal: bring current adapters up to the new contract before adding many new tools.

Tasks:

1. Claude Code:
   - preserve existing passive scanner
   - add resume command extraction
   - wire hook/statusline source health
   - add permission/live-state fixture coverage
   - never expose approve/interrupt
2. Codex:
   - standardize docs/config references to `[features].hooks`
   - add app-server bridge behind capability probe
   - expose `turn/interrupt` and approval response only as gated local operations
   - keep rollout/state fallback
   - add desktop deep link fallback to copy resume command
3. OpenClaw:
   - keep Gateway/session-store as source of truth
   - label cancel/delete as best-effort
   - expose Gateway ops only with auth/scopes/capability evidence
4. Hermes:
   - migrate from old sessions index assumptions to read-only `~/.hermes/state.db`
   - keep sessions index only as routing enrichment if still useful
   - add read-only SQLite/WAL tests
   - promote from Experimental after fixtures pass

Self-review:

- Current supported tools remain stable.
- Hermes promotion is test-backed, not docs-only.
- Codex app-server operations are local-admin only.
- No old config alias or silent hook write remains in docs.

Self-test:

- `cargo test -p octomonitor-adapter-claude` if available, otherwise workspace Rust tests focused by adapter module.
- Codex event/resume tests.
- Hermes SQLite fixture tests.
- Web InspectDrawer smoke tests.

## Phase 3 - P0 Monitoring Expansion

Goal: add high-value passive/managed adapters with strong evidence.

Implementation order:

1. CodeBuddy:
   - detect command and config dir
   - parse `~/.codebuddy/projects/**/*.jsonl`
   - use hooks `transcript_path` to validate transcript discovery
   - parse headless JSON/stream-json final statistics
   - read worker registry/liveness only as process health
   - expose verified worker logs/kill only when process is attested
2. Gemini CLI:
   - parse chat JSONL with metadata, `$set`, `$rewindTo`
   - handle corrupt/empty files
   - estimate cost only if pricing table exists
   - hook ingest for live state
3. Pi Agent:
   - parse JSONL tree sessions
   - support session-dir overrides
   - normalize usage/cost/provider/model
   - add managed RPC get_state/follow_up/abort later in operation layer
4. opencode:
   - use `opencode db path`
   - read SQLite WAL read-only
   - compare with CLI stats/export
   - deny auth/config secrets
5. GitHub Copilot CLI:
   - scan Chronicle/session-state and `session-store.db`
   - detect remote sync/privacy posture where available
   - ACP only after fixture
6. OpenHands:
   - parse conversation JSON
   - redact secrets in SDK persistence
   - map resume IDs
7. Continue `cn`:
   - parse sessions JSON
   - show usage if present
   - permissions YAML read-only health
   - logs diagnostics only
8. Qwen:
   - detect `~/.qwen/projects/<sanitized-cwd>/chats/*.jsonl`
   - fixture-gate passive parser
   - sidecar/hook live state
9. Kimi:
   - parse session index, state, wire JSONL
   - fixture-gate usage/tool event normalization
   - TOML hook support later
10. Goose:
   - read `sessions.db`
   - fallback to CLI list/export
   - handle legacy JSONL only with explicit import

Self-review:

- Each adapter has a clear confidence level.
- No adapter reads denied paths.
- Low-confidence usage does not enter global usage totals.
- Active state source is labeled: api/hook/process/passive/inferred.
- New adapters do not slow bootstrap significantly.

Self-test:

- Adapter fixture tests for every tool implemented in this phase.
- Workspace-level Rust tests.
- Web Usage and Monitor snapshots with mixed tools.
- Performance test with many JSONL/SQLite sources.

## Phase 4 - Fixture-Gated And Detection-Only Integrations

Goal: add safe detection or limited support without overclaiming.

Tasks:

1. Cline:
   - implement metadata-only SQLite parser after schema fixture
   - enforce `--auto-approve false` for managed runs
   - hide Hub ops until approval/request/send fixtures exist
2. Kiro:
   - implement CLI list/resume/delete display without delete action by default
   - implement custom-storage capture for OctoMonitor-managed sessions
   - DB scanner remains fixture-gated by OS/version
3. Cursor:
   - use official CLI bridge first
   - private store parser opt-in only
   - usage always `N/A` unless managed output proves otherwise
4. WorkBuddy:
   - detection-only
   - no hook install
   - no passive parser unless schema matches fixture
5. Amazon Q legacy:
   - detect old Q CLI
   - show migration note toward Kiro
   - no automatic migration or auth read
6. Aider:
   - optional workspace-local history helper
   - not a global source by default
7. Amp/Windsurf/Codebuff/Roo/Kilo:
   - detection/watchlist only unless official local session contract appears

Self-review:

- Candidate tools cannot look like stable supported sources.
- Detection-only tools do not produce false usage totals.
- UI explains why each candidate is limited.

Self-test:

- Detection tests with absent/present binaries.
- Negative parser fixtures.
- UI candidate-state snapshots.

## Phase 5 - Integration Settings And Source Controls

Goal: make integrations controllable without mutating external tools by default.

Tasks:

1. Add per-source preferences:
   - enabled/disabled
   - visible/hidden
   - allow passive scan
   - allow hook ingest
   - allow managed operations
2. Disable behavior:
   - stop probe
   - stop scan
   - stop watchers
   - stop hook ingest route for that tool
   - retain historical data but gray/hide based on preference
3. Show integration row state:
   - installed command
   - detected version
   - data root
   - store format
   - last seen
   - last scan
   - schema confidence
   - parse errors
   - hook status
   - operation capability count
   - privacy warning
4. Add "Run verification test" for fixtures and source health.

Self-review:

- Disable is real, not a UI-only filter.
- Remote viewer cannot alter integration settings.
- Preferences persist and survive probe refresh.

Self-test:

- Disable/enable for each source type.
- Probe refresh persistence test.
- Web interaction tests for Settings/Integrations.

## Phase 6 - Hook Manager

Goal: provide explicit, reversible hook installation.

Tool order:

1. Claude
2. Codex
3. Gemini
4. CodeBuddy
5. Qwen
6. Kimi
7. Kiro managed agent config
8. Hermes hook doctor/list surfaces where applicable
9. Cline managed hooks dir only after fixture

Tasks:

1. Build generic hook transaction:
   - detect
   - plan
   - preview semantic/raw diff
   - backup with sha256
   - atomic write
   - verify
   - audit
   - uninstall managed block only
2. Add format handlers:
   - JSON
   - TOML
   - hook directory/file
3. Add strict hook policy:
   - observe-only default
   - no HTTP hook default when command hook works
   - no permission auto-allow
   - no extra unknown keys for strict TOML formats
4. Add rollback UX and audit log.

Self-review:

- Hook Manager never writes without preview and confirmation.
- Existing third-party hooks are preserved.
- Configs with syntax errors are not silently repaired.
- Uninstall cannot remove unmanaged hooks.

Self-test:

- Install/verify/uninstall for each supported tool fixture.
- Hash mismatch rollback test.
- Syntax error config test.
- Secret redaction in preview.

## Phase 7 - Operation Layer

Goal: expose high-value operations safely.

Operation rollout:

1. Resume/open/copy command:
   - all tools with official resume command or ID
2. Codex app-server:
   - thread/list/read/resume
   - turn/start
   - turn/interrupt
   - approval response
   - inject/steer only if exact context is shown
3. Pi RPC managed:
   - get_state
   - prompt/follow_up
   - abort
4. Hermes ACP managed:
   - list/load/resume
   - cancel
   - permissions only after exact payload fixture
5. opencode server:
   - abort
   - permission respond
   - message send only for paired/managed server
   - deny shell/file/config/auth endpoints
6. CodeBuddy worker:
   - attach/logs
   - kill verified worker only
   - no arbitrary PTY input
7. Qwen sidecar:
   - submit
   - confirmation_response
   - managed-only, request-id scoped
8. Cline/Kimi/Copilot/OpenHands:
   - operations only after fixture/capability probe

Self-review:

- Every operation has capability descriptor and audit level.
- Mutating operations require confirmation.
- Approve/deny always shows exact payload.
- Kill requires PID/cwd/command/start-time attestation.
- Managed-only operations cannot run against passive historical sessions.

Self-test:

- Stale session ID.
- Stale PID/PID reuse.
- Missing auth.
- Expired capability.
- Denied remote mutation.
- Operation audit log.

## Phase 8 - Jump Links Lite

Goal: make "return to work" reliable without brittle focus hacks.

Tasks:

1. Add jump target provider model.
2. Implement:
   - copy command
   - open workspace
   - resume in new terminal tab
   - native deep links
   - focus managed terminal
3. Providers:
   - Warp URI
   - iTerm2 AppleScript/Python later
   - Ghostty AppleScript/capability probe
   - Terminal.app new tab/window only
   - VS Code open workspace
   - Cursor open workspace/copy resume
   - tmux managed sessions
4. Managed focus:
   - only when OctoMonitor launched or explicitly registered session
   - capture provider/window/tab/pane/pid/tty/cwd
   - revalidate before focusing

Self-review:

- Unmanaged sessions never show "focus existing terminal".
- Title matching is not used as a contract.
- Commands are quoted safely.
- Remote viewer does not expose local jump actions unless explicitly safe.

Self-test:

- Provider command rendering tests.
- macOS manual smoke matrix.
- Shell quoting tests.
- Missing provider fallback to copy command.

## Phase 9 - UI, Privacy, And Final QA

Goal: make the control plane understandable and safe.

Tasks:

1. Monitor:
   - source health badges
   - state source labels
   - usage confidence labels
   - action buttons from capabilities
2. Usage:
   - exact/estimated/partial/N/A grouping
   - exclude low-confidence sources by default
3. Inspect drawer:
   - transcript body gated by local explicit action
   - tool timeline
   - operation audit history
   - jump targets
4. Settings/Integrations:
   - support level
   - data root
   - schema confidence
   - hook status
   - privacy warnings
   - verification tests
5. Docs:
   - README support matrix
   - integration docs
   - safety model
   - fixture contribution guide

Self-review:

- No overpromising support level.
- Privacy warnings are visible but not noisy.
- Buttons are disabled or absent when capability is unavailable.
- Remote read-only boundary remains intact.

Self-test:

- Web build/test.
- A11y audit.
- Screenshot review desktop/mobile.
- Rust workspace tests.
- Manual local smoke with at least Claude/Codex plus two new adapters.

## Final Release Gate

Do not consider the implementation complete until all are true:

- Each stable target has fixtures and passing parser tests.
- Each mutating operation has audit and confirmation tests.
- Hook Manager install/uninstall passes for at least Claude, Codex, Gemini, and one non-existing-current-tool target.
- Remote router exposes no mutation.
- Usage totals exclude unknown/low-confidence sources by default.
- Source disable stops probe/scan/watch/ingest.
- Final docs match actual behavior.
- A final code review finds no blocker.

## Initial Self-Review Findings

Finding 1: The scope is broad enough to risk turning into a multi-month rewrite.
Correction: The plan keeps a foundation-first approach and requires evidence fixtures before adapter promotion. It does not require every watchlist tool to become a stable source.

Finding 2: Hook Manager could become unsafe if implemented as generic config writer.
Correction: The plan requires per-tool format handlers, preview, backup, atomic write, verify, audit, and managed-block-only uninstall.

Finding 3: Operation APIs could overstep the local-first boundary.
Correction: The plan limits operations to official APIs/RPC/ACP/sidecar or OctoMonitor-managed processes, with remote viewers read-only.

Finding 4: Usage aggregation could become misleading across partial sources.
Correction: The plan requires exact/estimated/partial/N/A labels and excludes low-confidence usage from totals by default.

Finding 5: Terminal jump could become brittle if old-tab focus is promised.
Correction: The plan limits precise focus to managed sessions with captured handles; unmanaged sessions get resume/new-tab/copy.

## Second Self-Review Findings

Finding 1: Kiro DB support was previously too strong.
Correction: Kiro is CLI/custom-storage first; DB scanner remains fixture-gated because exact DB filename/schema is not a public contract.

Finding 2: CodeBuddy evidence changed across reports.
Correction: CodeBuddy is included as a high-value target, but transcript per-line normalization and worker actions require golden fixtures before release.

Finding 3: Continue `cn` should not be left as managed-only.
Correction: Continue is Monitored-lite read-only because source-level evidence supports session JSON files, while operations remain minimal.

Finding 4: Candidate/detection tools could clutter the UI.
Correction: Candidate rows require explicit support labels, schema confidence, and no usage contribution unless enabled and fixture-gated.

Finding 5: Final implementation prompt must prevent stopping after one phase.
Correction: The generated prompt should require phase-by-phase implementation, review, self-test, fix, commit, then continue until complete or blocked by a concrete user decision.

## Third Self-Review Findings

Finding 1: Some fixture generation depends on external accounts, paid plans, regional availability, or closed-source binaries.
Correction: A tool may enter the implementation only at the highest level supported by local evidence. If a binary/account is unavailable, implement detection, fixture schema, adapter skeleton, and docs, but do not mark the adapter stable or include it in usage totals.

Finding 2: A full control-plane release could become blocked by proprietary tools that cannot be verified in the current environment.
Correction: Stable release criteria are per-adapter. The overall product can complete with fixture-gated/detection-only labels for unavailable tools, as long as the UI does not overclaim support and all implemented stable adapters pass their gates.

Finding 3: Hook Manager support across many tools could duplicate unsafe config editing code.
Correction: Hook Manager must use shared transaction machinery plus small per-tool format adapters. New hook targets are blocked until they use the shared detect/plan/diff/backup/atomic-write/verify/uninstall path.

Finding 4: Operation support may invite adding high-risk endpoints just because a server API exists.
Correction: Each operation must map to an explicit `OperationContract`. Shell, file write, config patch, auth, provider-key, and arbitrary PTY endpoints are denied even if an official API exposes them.

Finding 5: Watchlist tools could create product noise.
Correction: Detection-only tools should appear behind an "experimental/candidate sources" control or in diagnostics, not as first-class monitored rows unless the user enables candidate integrations.
