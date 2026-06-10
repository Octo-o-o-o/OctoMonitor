# Agent Control Plane Final QA

Date: 2026-06-10

This document records the implemented control-plane behavior after the phased upgrade. The phase plan remains the design history; this file is the final behavior checklist.

## Safety Gates

- Transcript bodies are local-only and require an explicit `Load transcript` action in the inspect drawer. Selecting a run can still show metadata already present in the live snapshot, but it does not call `/api/runs/{id}/events` or `/api/runs/{id}/inspect` until the user clicks.
- Remote viewer routes expose only `/api/bootstrap`, `/api/stream`, and `/api/pair/claim`. Remote bootstrap redacts workspace paths, transcript paths, session ids, thread ids, prompt/response bodies, capabilities, jump targets, local source paths, commit roots, and identity fingerprints.
- Hook Manager writes are explicit transactions: detect, plan, preview managed diff, backup, atomic write, verify, audit, uninstall. There is no silent hook write and no automatic approval widening.
- Operation Layer exposes descriptors first and executes only capability-gated actions. `open.workspace` requires local confirmation and last-activity attestation. Approval, interrupt, and kill remain blocked unless an exact payload and owned/attested process can be proven.
- Jump Links Lite renders copy-only targets for commands, terminal provider URLs/scripts, workspace paths, app CLI commands, and deeplinks. Existing terminal focus is only represented for managed sessions with attested identity.
- Source controls stop scan/probe/watch/ingest for disabled sources; hidden sources remain collected but are omitted from visible Monitor/Usage panels.

## Visibility Checks

- Monitor rows show source confidence/freshness and operation capability counts when present.
- Usage source cards show usage confidence, cost kind, usage source, and excluded bucket counts.
- Inspect shows source confidence, usage confidence, usage semantics, data-source health, operation capabilities, jump targets, and local operation affordances.
- Settings shows per-source enabled/visible controls, version/root/format/last-seen/schema confidence/parse errors/hook status/operation capability/privacy warnings, plus Run verification test.

## Integration Status

- Stable or monitored integrations are fixture gated. A tool without evidence fixtures must remain experimental, candidate, or detection-only.
- Detection-only/watchlist integrations intentionally do not expose mutation operations.
- Cursor private storage remains opt-in only and defaults to experimental metadata handling.

## Required Final Verification

- Rust workspace tests.
- Web unit tests.
- Web production build.
- Fixture evidence verification.
- Route audit through remote access tests.
- Accessibility/visual pass for the changed UI surfaces when a browser environment is available.
