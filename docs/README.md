# Documentation Map

This repository keeps current product docs, contributor docs, and historical plans side by side. Use this page as the entry point.

## Current Docs

- [README.md](../README.md): product overview, install paths, local run flow, desktop packaging, remote viewer basics
- [README.zh.md](../README.zh.md): Chinese version of the user-facing product docs
- [CONTRIBUTING.md](../CONTRIBUTING.md): contributor setup, architecture overview, conventions, adapter integration checklist
- [CLAUDE.md](../CLAUDE.md): condensed repo context (goal, commands, structure, current state, rules) for any coding agent working in this workspace. [AGENTS.md](../AGENTS.md) is a stub pointing here.
- [simplification-plan-2026-04-15.md](./simplification-plan-2026-04-15.md): scope decision log for the current product boundary
- [integration-support-audit-2026-06-10.md](./integration-support-audit-2026-06-10.md): current CLI support matrix and official-source research for Claude Code, Codex, OpenClaw, Hermes, Gemini CLI, Cursor Agent, WorkBuddy/CodeBuddy, and Pi Agent

## Active Plans

- [plan/2026-06-10-agent-control-plane-roadmap.md](./plan/2026-06-10-agent-control-plane-roadmap.md): target-state roadmap for turning OctoMonitor into a local-first CLI agent control plane with monitoring, live state, operations, hook management, and jump links
- [plan/2026-06-10-agent-control-plane-implementation-plan.md](./plan/2026-06-10-agent-control-plane-implementation-plan.md): phase-by-phase implementation plan for the full control-plane upgrade, including evidence locks, adapters, Hook Manager, operations, jump links, QA gates, and self-review findings

## Runtime Surfaces

- Local admin API and bundled web shell: `127.0.0.1:46321`
- Web dev server: `127.0.0.1:4173`
- Remote read-only viewer: `0.0.0.0:46322`, only when enabled from `Settings -> Remote Access`

## Historical Docs

Superseded design notes, implementation plans, and older visual explorations live under [`docs/history/`](./history/).

These files are still useful for rationale, but they are not the source of truth for current behavior. Of note:

- [`history/reference-monitoring-inspiration.md`](./history/reference-monitoring-inspiration.md) and [`history/implementation-plan-monitoring-inspiration.md`](./history/implementation-plan-monitoring-inspiration.md): Phase 0-3 已落地（CopyButton/Toast、resume-command API、Codex 事件 parser、MonitorFilterBar、events endpoint、InspectDrawer timeline 等）。归档保留作为已完成的实施记录；Phase 4+ 仍可作为后续评估的策略参考。

## Maintenance Rules

- Avoid exact test counts or other fast-drifting status snapshots in long-lived docs.
- When a plan is completed or abandoned, move it under `docs/history/` instead of leaving it in the main `docs/` root.
- Keep `README.md` and `README.zh.md` aligned for user-facing behavior.
- If a historical note conflicts with current behavior, prefer `README.md`, `README.zh.md`, and `CONTRIBUTING.md`.
