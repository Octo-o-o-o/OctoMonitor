# Agent Control Plane Operations

Phase 7 adds a local operation API with conservative execution gates.

Local routes:

- `GET /api/runs/{run_id}/operations`
- `POST /api/runs/{run_id}/operations`

Currently executable:

- `resume.copyCommand`: returns the existing advisory resume command for tools that expose a safe resume id.
- `open.workspace`: opens the run workspace through the local OS only when the request includes `confirmed: true`, the run still matches `expectedLastActivityAt`, and the path exists.

Explicitly blocked until stronger evidence exists:

- `turn.interrupt`: requires an attested managed app-server turn target.
- `approval.respond`: requires the exact native approval payload.
- `process.kill`: requires owned PID/cwd/command/start-time attestation.
- RPC/sidecar/server send operations for Pi, Hermes ACP, opencode, CodeBuddy, Qwen, Cline, Kimi, Copilot, and OpenHands remain capability-gated until fixtures prove request scoping and rollback behavior.

Safety boundaries:

- Stale run guard: mutation requests can include `expectedLastActivityAt`; mismatches are blocked and audited.
- Confirmation gate: opening local workspaces requires `confirmed: true`.
- No arbitrary terminal stdin injection.
- No broad approve/deny route.
- No cross-tool kill route.
- The remote viewer router does not expose operation routes.
- Every attempt writes `~/.octomonitor/operation-audit.jsonl` metadata without raw prompts, responses, approval payloads, API keys, OAuth files, or `.env` data.

The Inspect drawer uses this API for the "Open workspace" action and keeps existing copy/deep-link fallbacks for resume and Codex desktop opening.
