# Native UI layout and zoom

Updated 23 October 2026. Visual polish is the current first priority.

## Header and workspace

The separate project toolbar and large scenario heading are combined into one
row. The title has a bounded width and an ellipsis; project context sits below
it. The description is available in Settings and the title tooltip. Run controls
and Settings remain visible; Import/Export are in the ellipsis menu with their
existing availability rules and secret-stripping export behavior.

At a 1600×1000 client size, the primary header and tabs occupy 100 logical pixels,
and the canvas toolbar another 43. Previously these occupied 248 pixels together:
the graph gains 105 pixels of vertical space. At 960×540, the primary header and
tabs occupy 94 pixels. A real notice can temporarily add height.

The canvas caption is one line; its hover tooltip explains execution mode. The
footer provides explicit zoom percentage, minus/plus buttons and Fit. The minimap
still disappears when the board is too small.

## Typography and spacing

The embedded API Forge Sans regular/semibold fonts use the same metrics across
platforms. Adwaita Mono is used for code, and DejaVu Sans remains the symbol
fallback. Sources and the OFL license are bundled beside the fonts.

Shared theme values define 8/12/16/24-pixel spacing, 13-pixel body text,
12-pixel labels, 11-pixel metadata and 20-pixel headings. Form inputs and primary
controls use a 32-pixel target height. Panels use cool neutral surfaces, subtle
borders and the existing orange accent. Library tabs have equal-width cells;
navigation and library text truncate to measured available width.

## Card zoom

Card geometry and connection anchors remain stable while visual detail changes:

| Zoom | Visible content |
| --- | --- |
| Below 45% | One centered, truncated title, kind accent and ports |
| 45% to below 75% | Title, argument values, output names and ports |
| 75% and above | Full captions, argument labels, types and footer actions |

Inline editing starts at 80%; Inspector remains available at every zoom. Each
painted label has its own width budget, single-line ellipsis and clipping cell.
Full and compact card fonts scale with the graph rather than applying a minimum
font size that exceeds the available space. Overview titles have a separate
tile-sized cell. Dense distant grid dots are reduced, and tiny branch labels hide.

## Verification

The local workspace passes 238 Rust tests, Clippy with warnings denied, formatting,
the TypeScript compatibility fixture check and a workspace debug build. New
regressions check long and unbroken Unicode card labels at 25/35/45/60/75/100/160%
zoom, and the header height with a long title/description at 960×540, 1280×800 and
1600×1000, with Inspector open.

Actual KWin captures of the current build are retained under
`.gui-qa/runs/ui-polish-final/`: `workspace.png`, `inspector.png`,
`zoom-medium.png` (49%), `zoom-minimum.png` (25%), `compact.png` (960×540), and
`settings.png`. The private KWin session stopped cleanly. These screenshots verify
layout; they do not establish complete interaction or accessibility acceptance.

A second direct KWin run under `.gui-qa/runs/ui-polish-interaction/` verifies the
project menu entries, Ctrl+K selection, an Inspector rename and Ctrl+Z restoration
(including saved workspace readbacks), distant zoom, compact Settings and a
successful Simulation run showing 8 passed steps at 960×540. It also stopped its
private session cleanly. Native file portals were not launched.

The Gemini run `20261023T163339Z-4d44fa8b` started KWin and inspected the header,
then stopped with the account's `RESOURCE_EXHAUSTED` quota error. It is incomplete
and is not a GUI PASS. The standalone KWin configuration is retained; the direct
interaction run above supplies the completed local regression evidence.
