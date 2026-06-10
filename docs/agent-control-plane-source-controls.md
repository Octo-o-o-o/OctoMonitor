# Agent Control Plane Source Controls

Phase 5 adds server-backed source controls for each monitored or candidate agent tool.

## Collection Gate

`AppConfig.disabledSources` is the collection gate. A disabled source is excluded before adapter command probes, filesystem scans, P0 passive probes, watcher subscriptions, and hook/statusline ingest writes. Existing in-memory runs, identities, adapter health rows, and OpenClaw/Hermes cron rows for that source are pruned only after the config patch is persisted successfully.

## Visibility Gate

`AppConfig.hiddenSources` is display-only. Hidden sources continue to collect data, but Monitor and Usage omit them from source columns and usage groupings. This is intentionally separate from `disabledSources` so users can hide noisy sources without losing history collection.

## Settings Verification

Settings includes a read-only "Run verification test" action backed by `GET /api/installer/verify`. The endpoint returns current doctor checks, disabled/hidden source lists, and adapter health rows. It does not mutate tool config files or write hooks.

## Safety Notes

- Source toggles patch only OctoMonitor config.
- Disabled sources ignore local hook/statusline ingest requests.
- Watcher reconciliation unsubscribes disabled source directories and resubscribes when re-enabled.
- Remote bootstrap redaction preserves source-control state while continuing to hide local IP details.
