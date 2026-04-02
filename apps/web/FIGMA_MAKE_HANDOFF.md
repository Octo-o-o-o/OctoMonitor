# OctoMonitor Figma Make Handoff

## Recommended input

Use the dedicated mock page instead of pasting the production app code directly.

- Entry page: `apps/web/figma-make.html`
- Main page component: `apps/web/src/figma-make/FigmaMakeExport.tsx`
- Mock content: `apps/web/src/figma-make/mockData.ts`
- Styles: `apps/web/src/figma-make/styles.css`

This handoff page is intentionally different from the live app:

- It removes websocket, store, API, and drawer state.
- It flattens hidden tabs and drawers into one long canvas.
- It keeps OctoMonitor's real information architecture and vocabulary.
- It includes edge cases that screenshots usually miss.

## Why this format is best

For this project, the best Figma Make input is:

1. A runnable TSX page with semantic structure.
2. Mock data embedded in a simple, readable file.
3. A short prompt describing the visual direction and constraints.

Less effective options:

- Raw production code: too much runtime logic, not enough visual clarity.
- Screenshot only: preserves appearance but loses hierarchy and component semantics.
- Pseudocode only: preserves intent but loses concrete layout and content density.

## Suggested prompt

Use the attached TSX and CSS as a structural reference for OctoMonitor. Redesign it into a sharper, more polished desktop-first monitoring UI that still works on mobile. Preserve the product information architecture: monitor board by source, selected run detail, usage analytics, settings, identities, and installer states. Keep approval-needed and offline states visually prominent. Use same-origin safe language for companion mode and do not add fake features like cloud sync or multi-tenant dashboards.
