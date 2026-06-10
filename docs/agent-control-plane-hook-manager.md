# Agent Control Plane Hook Manager

Phase 6 adds an explicit, reversible Hook Manager for observe-only live state.

Supported install targets are fixture- and documentation-backed:

- Claude Code: `~/.claude/settings.json` or `CLAUDE_CONFIG_DIR/settings.json`
- Codex: `~/.codex/hooks.json` or `CODEX_HOME/hooks.json`
- Gemini CLI: `~/.gemini/settings.json` or `GEMINI_HOME/settings.json`
- CodeBuddy: `~/.codebuddy/settings.json` or `CODEBUDDY_CONFIG_DIR/settings.json`
- Qwen Code: `~/.qwen/settings.json` or `QWEN_CONFIG_DIR/settings.json`

Kiro, Kimi, Hermes, opencode, and Cline are exposed as unsupported/detection-only in the Hook Manager until a safe global target or fixture-backed plugin transaction exists.

The transaction is:

1. Detect current target state.
2. Build an install or uninstall plan.
3. Show a redacted diff that includes only the OctoMonitor-managed hook command.
4. Require explicit user apply with the previewed `beforeSha256`.
5. Write a backup under `~/.octomonitor/hook-backups/<tool>/` when changing an existing file.
6. Atomically write the JSON config.
7. Verify the managed hook is present or absent.
8. Append a local audit line to `~/.octomonitor/hook-audit.jsonl`.

Safety boundaries:

- No silent hook writes. The server only writes after `POST /api/hooks/{tool}/apply` with a matching preview hash.
- No broad approve or deny behavior. The installed command prints an empty JSON object and never returns an approval decision.
- No native HTTP hook handler is installed by default. Tools receive a command hook, and that command forwards filtered metadata to local loopback.
- Existing third-party hooks are preserved. Uninstall removes only hook handlers containing the OctoMonitor marker.
- Preview output avoids echoing existing user hook commands. It only shows the managed OctoMonitor command that will be added or removed.
- Generic hook ingest stores event metadata only: event name, session/thread id, cwd, transcript path, model, and waiting-approval hint. It does not store raw hook payloads, prompts, responses, tool inputs, OAuth files, API keys, or `.env` data.

Local API surface:

- `GET /api/hooks`
- `GET /api/hooks/{tool}/plan?action=install|uninstall`
- `POST /api/hooks/{tool}/apply`
- `POST /api/hooks/ingest/{tool}/hook`

The remote access router does not expose these mutation routes.
