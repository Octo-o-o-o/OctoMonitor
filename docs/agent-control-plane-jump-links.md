# Agent Control Plane Jump Links Lite

Phase 8 adds capability-derived jump targets to each local run.

Supported target types:

- `copyCommand`: copy a resume command for tools with a safe resume id.
- `workspace`: expose the workspace path; opening it still goes through the Phase 7 audited `open.workspace` operation.
- `nativeApp`: copy editor launch commands for VS Code (`code <folder>`) and Cursor (`cursor <folder>`).
- `newTerminalTab`: copy provider-specific launch targets for Warp, iTerm2, Ghostty, and Terminal.app. OctoMonitor does not execute these commands from the web UI.
- `sessionDeeplink`: expose official app deep links such as `codex://threads/<id>`.
- `managedTerminalFocus`: emitted only when a run carries attested terminal identity (`windowId`, `tabId`, `paneId`, or `pid`). Unmanaged sessions never get this target.

Safety boundaries:

- No arbitrary terminal stdin injection.
- No title-based focus matching.
- No unmanaged old-tab focus.
- No remote viewer exposure of local jump actions; remote redaction keeps `jumpTargets` empty.
- Shell commands are represented as argv arrays and rendered with shell quoting in the UI.

Provider notes:

- Warp uses the official URI form `warp://action/new_tab?path=<folder>`.
- iTerm2 and Terminal.app targets are AppleScript command strings for a new window/session, not existing-tab focus.
- Ghostty targets use the documented `working-directory`/`-e` style command surface and remain lower reliability because behavior differs by platform/version.
- VS Code workspace opening follows the official `code <folder>` CLI pattern.

References:

- Warp URI Scheme: <https://docs.warp.dev/terminal/more-features/uri-scheme/>
- iTerm2 Scripting: <https://iterm2.com/documentation-scripting.html>
- Ghostty Configuration Reference: <https://ghostty.org/docs/config/reference>
- Ghostty 1.3.0 Release Notes: <https://ghostty.org/docs/install/release-notes/1-3-0>
- VS Code Command Line Interface: <https://code.visualstudio.com/docs/configure/command-line>
