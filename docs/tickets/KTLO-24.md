# KTLO-24: The bar with the open files hides itself when no file is open

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Approved 2026-10-27 (issue #57) — decisions 1 and 2 = A taken
  2026-10-26; decision 3, the mock-up as a whole, approved 2026-10-27
- Analysis model: Fable · Implementation model: Opus 5.5
- Mock-up branch: `mock/ktlo-24-hide-tab-strip` (throwaway; deleted after the
  decision)
- Owner's wish of 2026-10-26.

## Goal

When every file is closed, the grey band under the app header that holds the
open files' tabs is not there at all, so the code area or the board starts
right under the app header; the band comes back the moment a file is opened.

## What the owner will see

- **New:** nothing new is drawn. With no file open, in Code, Diagram and
  Split, the band disappears with its margins (28 px plus 8 px of margin
  today), and the project files panel, the code area or the board move up
  by that much to meet the app header. Opening a file from the project files
  brings the band back with that file's tab; closing the last tab (its cross
  or Ctrl+W) takes it away again.
- **Changed:** when a notice belongs in that band while no file is open —
  "Unsaved work from an earlier session is in the recovery backup", an
  orphan draft, unreadable recovery data, the storage label with its
  spinner, the project-file notices (changed, invalid, missing) or the
  dismissable notice with its cross — the band comes back carrying only the
  notice, and closes up again when the notice goes. With one or more files
  open, nothing changes: the strip, its tabs, crosses and notices are as
  today.
- **Removed or hidden:** the empty band, decision 2. It carries no control
  while it is empty, so nothing reachable is lost.
- **Migrated or converted:** nothing.
- **Default behaviour that changes:** a freshly opened project with no file
  open starts without the band (today: with an empty grey strip). No key,
  window or panel position changes.

## Mock-up

Captured on `mock/ktlo-24-hide-tab-strip` (commit `ae6306c`, 2026-10-27)
under `docs/qa/2026-10-26/KTLO-24/`: `mock-code-empty-960x540.png`,
`mock-diagram-empty-960x540.png`, `mock-split-empty-960x540.png`,
`mock-diagram-empty-1600x1000.png` (the band gone, the hint or the dog of
KTLO-20 directly under the app header), `mock-diagram-notice-960x540.png`
(no file open, the band back with one notice) and
`mock-diagram-one-tab-960x540.png` (one file open, the strip as today).
Posted on the issue.

Measured on the mock-up at 960 × 540 against `main`: the empty board starts
at y = 57 instead of 94, so 37 px come back — the 36 px band and the panel's
1 px separator. With one tab open the band is 40 px on both, so opening the
first file moves the board down by 41 px. With a notice and no tab the band
is 46 px (74 px on `main`, where the empty strip sits above the notice).

Rendering notes: the mock-up is a `review_` view of the folder modes with
every tab closed (`testing::review` opens the demo project with a tab, so the
view closes it first), as [design review](../gui-testing.md#design-review)
describes; the notice view sets a dismissable notice before the capture.

## Decisions for the owner (taken: 1 = A, 2 = A on 2026-10-26; 3 approved on 2026-10-27)

1. **When does the band hide?**
   - **A (recommended, taken): at zero open files only.** With exactly one
     file open the strip stays, so the file's name and its close cross are
     reachable whenever anything is open.
   - **B (declined): at one file or none.** 36 px more for the one open
     file, but its name and its cross would vanish from the window, leaving
     Ctrl+W or the project-files panel to close it.
2. **What hides?**
   - **A (recommended, taken): the whole band.** The `folder-tabs` band with
     its own 4 px margins takes no height at all; the app header sits
     directly on the code or the board. A notice brings the band back for as
     long as it is shown.
   - **B (declined): the band stays at its height as an empty strip** —
     effectively today's behaviour.
3. **The mock-up** as a whole: the three modes without the band at both
   sizes, the band back with a notice, and the strip unchanged with one tab.

## Acceptance criteria

1. In Code, Diagram and Split with no tab open, no tab, no *Close …* button
   and no node of the tab strip is in the accessibility tree, and the top
   edge of the central area — the project files panel and the code area in
   Code, the board in Diagram, the code panel and the board in Split — is
   equal to the bottom edge of the app header (±1 px) (interaction test at
   960 × 540 and 1600 × 1000).
2. Opening a file from the project files panel while no tab is open brings
   the band back: the file's tab and its *Close …* button are in the tree and
   the central area starts under the strip, at the height it has today
   (40 px with a tab open, measured on the mock-up).
3. Closing the last tab with its cross, and with Ctrl+W, leaves the window
   as in criterion 1; a freshly opened project with no file open starts as
   in criterion 1.
4. With no tab open and a notice shown (the dismissable notice set by
   `notify`, and the recovery-backup notice from a fixture that has one), the
   band is back carrying the notice and no tab; dismissing the notice
   returns to criterion 1 (both sizes).
5. With one tab open, and with more tabs than fit the width, the strip, its
   crosses, the active tab's reveal and the notices under it behave exactly
   as today: the existing tab-strip and KTLO-12 header tests pass unchanged.
6. The accessibility audit passes in all three modes with no tab open, and
   Tab from the app header lands on the first control of the central area
   instead of on the strip.

## Not in scope

The hint and the sleeping dog of the empty board (KTLO-20, #46); the empty
Code view's hint; the one header above the board (KTLO-12, #16) and the
maximised run results panel (KTLO-19, #44), whose top edge follows the band
and is not changed here; the Problems section (KTLO-18, #43); the library
rail and project files in Diagram (KTLO-17 #42, KTLO-11 #14); any change to
the notices' text or placement while a tab is open; an animated collapse.

## Technical appendix

- Today: `FolderApp::tab_strip` (`crates/apiforge-desktop/src/folder/chrome.rs:150`)
  returns early with `ui.add_space(theme::CONTROL_HEIGHT - 4.0)` — 28 px —
  when `workbench.tabs()` is empty, and clears `followed_tab`. The band is
  `egui::Panel::top("folder-tabs")` in `folder/mod.rs:623-640`, frame
  `theme::PANEL`, `inner_margin` symmetric (8, 4); its closure calls
  `tab_strip` then `notices`.
- The crux: an egui top panel takes its height from what it draws, so the
  panel must not be shown at all when there are neither tabs nor notices.
  `notices` (`chrome.rs:257`) decides inside itself what it draws
  (`storage.label()`, `recovery_decision_required`, `recovery_unavailable`
  with `!is_read_only`, orphan drafts, `registry_state() != Current`,
  `self.notice`). Split that decision out into a `fn has_notices(&self) ->
  bool` with the same conditions, and in `mod.rs` show the panel only when
  `!workbench.tabs().is_empty() || self.has_notices()`; drop the
  `add_space` in `tab_strip` (keep the `followed_tab = None`). The band then
  has no height to remember, so no panel id epoch is needed.
- `tabs_width` and `tabs_offset` are only written when the strip is drawn;
  nothing reads them in the empty state. The mock-up built exactly this
  recipe and the 361 existing desktop tests passed with the band hidden.
- `notify` is private to the `folder` module; the criterion 4 test needs a
  test-visible way to set the dismissable notice (the mock-up widened it to
  `pub(crate)` for its review view).
- Tests: `folder/acceptance_tests.rs` for criteria 1–4 and 6 — the app
  header's rect via its `"Project"` button row (as `header_tests.rs:106`
  does), the central area via `"Diagram board"`, the project files panel
  and the code hint; close the demo project's tab with `Close <file>`.
  `header_tests.rs:106` (`one_row_lies_between_the_tab_strip_and_the_board`)
  asserts a row *below the tab strip* with a tab open; the fixture opens one,
  so it keeps passing, and criterion 5 pins that. The audit in
  `accessibility_tests.rs` gets the empty state as one more screen.
  `testing.rs` gets the `review_` views of the mock-up.
- Nothing in the view state (`panels.*`) refers to the band's height.
