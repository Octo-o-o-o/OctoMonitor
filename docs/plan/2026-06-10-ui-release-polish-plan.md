# OctoMonitor UI / Island / Release Polish Plan

Date: 2026-06-10
Status: implementation checklist for the final polish pass

## Goal

This pass is not a new feature wave. It is a product-quality pass over the current OctoMonitor dashboard, Island surface, desktop integration, copy, and release artifact. The output should be a small, high-confidence set of improvements that make the existing "Dashboard + Island + Codex jump" flow feel intentional.

## Non-Negotiables

- Local-first and read-only remain unchanged.
- `codex://` and desktop window commands stay local desktop only. They must not enter the remote router or ordinary browser surfaces.
- The Island window must remain `hide/show`, not destroy/recreate.
- Closing or collapsing Island must never terminate the bundled server.
- No new app modes, jump adapters, approval controls, account-level settings, or large layout rewrites in this pass.

## External Reference Notes

- Apple exposes notched-display safe layout through `NSScreen.safeAreaInsets` and the auxiliary top-left/top-right screen areas. The Island must keep using native screen metrics instead of hardcoding a model matrix.
  - https://developer.apple.com/documentation/AppKit/NSScreen/auxiliaryTopLeftArea-uglc
  - https://developer.apple.com/design/human-interface-guidelines/layout
- Vibe Island documents the same product behavior we want to match: built-in notched displays use the notch area; external displays and older Macs fall back to a top-center floating bar.
  - https://vibeisland.app/
- Vibe Island's public changelog calls out panel width customization at `440-800pt`, expanded height ranges, live preview, and notch-alignment tuning for edge cases. OctoMonitor's current default expanded width/height remain conservative at `640pt`/`560pt`, with a `277pt x 32pt` collapsed capsule and native notch measurement.
  - https://vibeisland.app/changelog/

## Page And Component Audit

### Main Shell

- Tabs: Monitor, Usage, Commits, Heatmap, Settings.
- Desktop menu actions already open Settings and shortcuts.
- Remote viewer correctly hides local-only Settings.
- Copy should stay terse and utilitarian; this is an operational tool, not a marketing surface.

Action: no structural change. Verify tab navigation and local-only settings with browser.

### Monitor

- Purpose: dense live status by source.
- Current grouping by source/project is still right for dashboard scanning.
- Completed rows already show visited/unvisited state.
- Clicking a run marks it visited and opens InspectDrawer.

Action: no dashboard grouping rewrite. Island is the surface that should flatten across projects.

### InspectDrawer

- Purpose: details, Codex timeline, copyable identifiers, Codex jump.
- "Open in Codex" should remain visible only in local Tauri and only when `run.threadId` exists.
- Fallback copy should remain visible under the jump button.

Action: verify copy and button hierarchy in browser. No remote exposure.

### Usage / Commits / Heatmap

- Purpose: history and attribution. These are already full-dashboard analysis surfaces.
- Avoid adding Island-specific actions here.
- Existing segmented controls and date controls fit the information density.

Action: browser walk through for overflow, labels, and empty states only.

### Settings

- Desktop Display section is now the key entry point for Island.
- The Island expanded header settings button should open the main dashboard and land on Settings.
- Display Mode and Island Position copy should explain surfaces without implying remote/browser support.

Action: verify settings section and desktop command route.

### Island Surface

This is the main improvement target.

Collapsed:
- Must fit the notch and external top-center fallback.
- Show only one priority-aware count, not a full dashboard summary.
- Priority order for the collapsed number: needs action, active, just completed. A quiet state can show `0`.

Expanded:
- Must flatten all tasks; do not group by project.
- Sort by status bucket first: needs action, active, just completed, completed.
- Sort within each bucket by `lastActivityAt` descending.
- First line should be the task/question/recent action, not the project name. Project/workspace/tool belongs in secondary text.
- Right meta should show compact status + last update recency. Duration is less useful than recency on this surface.
- The top-right settings button should be obvious but not steal visual priority.
- Clicking outside the expanded panel should collapse immediately.

Action: implement title/subtitle/recency refinement and verify hover/collapse/settings behavior.

## Browser / Desktop Walkthrough Checklist

1. Start local server and web dev server.
2. Browser: dashboard tabs at desktop width.
3. Browser: Monitor row states, visited/unvisited done row, InspectDrawer, Open in Codex visibility in non-Tauri browser.
4. Browser: Settings > Desktop Display layout and copy.
5. Browser: Island preview via `/?surface=island`; check collapsed and expanded layout.
6. Browser mobile/narrow pass for Monitor and Island preview.
7. Desktop/Computer Use: launch built app, verify local bundle, server health, Island hover/collapse does not kill server.
8. Desktop settings command: verify expanded Island settings path by unit coverage and, when the desktop session is visually available, by Computer Use.

## Implementation Scope For This Pass

1. Refine Island content hierarchy:
   - task/action as title
   - project/workspace/tool as subtitle
   - compact last-update recency in meta
   - full last update in `title`
2. Strengthen Island tests:
   - expanded rows expose action/running/done labels
   - ordering remains status bucket + latest activity
   - settings command still collapses and invokes desktop command
3. Keep dashboard pages structurally unchanged unless browser walk finds concrete layout or copy regressions.
4. Update this plan with walkthrough findings before implementation.
5. Run full verification:
   - `pnpm --filter @octomonitor/web test --run`
   - `cargo test --workspace`
   - `pnpm --filter @octomonitor/web build`
   - `pnpm test:a11y`
   - `pnpm build:desktop`
6. Commit and push.
7. Build notarized DMG with `pnpm build:desktop:notarized`; validate with `spctl` and `stapler`.

## Walkthrough Findings

- Browser desktop Monitor:
  - Source columns, attention banner, quick filters, and row state badges render correctly.
  - Non-Tauri browser does not expose the local-only Codex jump action, which matches the security boundary.
- Browser Settings:
  - Desktop Display section is discoverable and the existing copy is understandable.
  - Display Mode and Island Position controls are already in the right section; no extra setting is needed.
- Browser narrow viewport:
  - Monitor switches to source tabs without horizontal overflow at 390px width.
  - Island preview does not create horizontal overflow at 390px width.
- Island preview:
  - Before this pass, the expanded list was correctly flattened and status-sorted, but each row's first line was still the project name. This weakened the "task list" model.
  - Before this pass, the actual task/question lived in the subtitle and the right meta showed duration instead of update recency.
  - The settings button is present with the right accessible name.
- Desktop visual walk:
  - Release `.app` launches with the bundled server when started from a clean environment.
  - `Computer Use` enumerates the built OctoMonitor app as running, but cannot capture a normal key window for the panel-style surface (`cgWindowNotFound`).
  - `screencapture` returns a black image in this desktop session, so pixel-level visual validation is blocked by the current screen capture environment.
  - System-event hover over the top-center island area followed by an outside click leaves both the app process and bundled server alive; `/api/health` remains OK.

## Final Implemented Changes

- Island expanded rows now put the most actionable task text first: question, recent action, tail text, then project/workspace/tool fallback.
- Island secondary text now carries project/workspace/tool context so the list remains flattened by task instead of grouping by project.
- Island right meta now shows compact last-update recency with the full formatted timestamp in the tooltip, matching the status-bucket and `lastActivityAt` sorting model.
- Island tests now assert action-first titles, project/workspace subtitles, status labels, recency metadata, notch metrics, hover expansion, outside-click collapse, and settings command behavior.
- Dashboard, Settings, remote/browser visibility, and local-only Codex jump boundaries did not require structural changes in this pass.
