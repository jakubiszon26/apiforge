# KTLO-24: The bar with the open files hides itself when no file is open — QA report

- Branch: `feat/ktlo-24-hide-tab-strip` (the PR head commit)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: `docs/tickets/KTLO-24.md` (on the docs branch
  `docs/ktlo-24-hide-tab-strip`, PR #58) and the `mock-*.png` in this
  directory, copied from `mock/ktlo-24-hide-tab-strip` (commit `ae6306c`);
  decision 1 = A (hide at zero open files only), decision 2 = A (the whole
  band), the mock-up approved as a whole on 2026-10-27
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked` (isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`;
  370 desktop tests, 9 of them new) and `apiforge-convert-v1 --fixtures
  --check`: all pass. The nine new tests passed ten runs in a row; the first
  ten-run loop had found the Tab walk of criterion 6 failing twice, which a
  test-helper fix (waiting for the view-state write) removed, then fifteen
  runs of that test and the ten-run loop passed.

## Captures

`after-*.png` are from the **release binary** (`--screenshot … --folder …
--mode … --size …` under Xvfb with Lavapipe), on the demo shop that
`--managed` created in an empty data directory.

A screenshot cannot click, so each state was set up on disk before the
capture:

- **One tab:** the project as `--managed` created it; the window opens its
  scenario.
- **No file open:** `tabs` emptied and `active` set to `null` in the
  project's `.apiforge/local/workspace.json`, which is what the window writes
  after its last tab is closed (scenario 3); the binary read it and wrote it
  back unchanged apart from the panel widths.
- **A notice with no file open:** the recovery-backup notice. The project's
  `recovery.json` was overwritten with ten bytes of truncated JSON and a valid
  `recovery.json.bak` holding no drafts was written beside it, as an
  interrupted recovery write leaves them. The mock-up shows the dismissable
  notice instead ("Moved to the trash."), which only an action inside the
  window can raise; both are covered by tests (scenario 4).

| View | After (release binary) | Approved mock-up |
| --- | --- | --- |
| Code, no file open, 960 × 540 | [after](after-code-empty-960x540.png) | `mock-code-empty-960x540.png` |
| Code, no file open, 1600 × 1000 | [after](after-code-empty-1600x1000.png) | — |
| Diagram, no file open, 960 × 540 | [after](after-diagram-empty-960x540.png) | `mock-diagram-empty-960x540.png` |
| Diagram, no file open, 1600 × 1000 | [after](after-diagram-empty-1600x1000.png) | `mock-diagram-empty-1600x1000.png` |
| Split, no file open, 960 × 540 | [after](after-split-empty-960x540.png) | `mock-split-empty-960x540.png` |
| Split, no file open, 1600 × 1000 | [after](after-split-empty-1600x1000.png) | — |
| Diagram, a notice, no file open, 960 × 540 | [after](after-diagram-notice-960x540.png) | `mock-diagram-notice-960x540.png` |
| Diagram, a notice, no file open, 1600 × 1000 | [after](after-diagram-notice-1600x1000.png) | — |
| Diagram, one tab, 960 × 540 | [after](after-diagram-one-tab-960x540.png) | `mock-diagram-one-tab-960x540.png` |

Measured on the release captures at 960 × 540 (the native window has no
harness margin, so its rows are 8 px higher than the mock-up's): the app
header is rows 0–47 with its separator at row 48, and with no file open the
board, the project files panel and the code panel start at row 49. With one
tab open the band is rows 49–88 (40 px), its separator row 89 and the board
starts at row 90, so opening the first file moves the board down 41 px. With
the recovery-backup notice and no tab the band is rows 49–94 (46 px), as the
mock-up's band with its notice. In the harness the same window measures the
central area at y = 94 on `main` and at y = 57 on this branch: 37 px come
back, the 36 px band and its 1 px separator.

## Linux

### Scenario 1: no file open, in every mode (criterion 1)
1. Open the demo shop at 960 × 540 and at 1600 × 1000.
2. Close its only tab; show Code, Diagram and Split.

Expected: no tab, no *Close …* button and no other node of the strip in the
accessibility tree; the project files panel and the code area (Code), the
board (Diagram), the code panel and the board (Split) start at the bottom
edge of the app header, ±1 px.
Observed: as expected and as in the mock-up (`after-*-empty-*.png`); in the
harness every part's top edge equals the header's bottom edge, y = 57. Test:
`with_no_tab_open_the_central_area_meets_the_app_header`.

### Scenario 2: opening a file brings the band back (criterion 2)
1. With no tab open, click `promotion-and-cart-price.afs` in the project files
   panel; show each mode.

Expected: the file's tab and *Close promotion-and-cart-price.afs* are in the
tree, on the band; the band is 40 px with its 1 px separator below it, and the
central area starts under it.
Observed: as expected (`after-diagram-one-tab-960x540.png` matches
`mock-diagram-one-tab-960x540.png`). Test:
`opening_a_file_from_the_project_files_brings_the_band_back`.

### Scenario 3: closing the last tab, and reopening (criterion 3)
1. In each mode at both sizes, close the only tab with its cross; in a fresh
   window, close it with Ctrl+W.
2. Close a project with no file open and open it again.

Expected: each time the window is as in scenario 1.
Observed: as expected; the reopened project restores its mode and opens
without the band. Tests:
`closing_the_last_tab_by_its_cross_or_ctrl_w_takes_the_band_away`,
`a_project_opened_with_no_file_open_starts_without_the_band`.

### Scenario 4: a notice with no file open (criterion 4)
1. With no tab open, raise the dismissable notice ("Moved to the trash.");
   show each mode; click *Dismiss*.
2. Open a project whose recovery file is unreadable beside a backup with a
   draft (the test writes two drafts through the session, then truncates the
   primary file); close its tab; show each mode; click *Restore backup*.

Expected: the band comes back carrying the notice as its first row and no
tab; the central area starts under it; dismissing or restoring takes the band
away again.
Observed: as expected at both sizes. On `main` the notice sat under an empty
28 px strip, 38 px below the band's top; it is now the band's first row.
Tests: `a_dismissable_notice_brings_the_band_back_until_it_is_dismissed`,
`the_recovery_backup_notice_brings_the_band_back_until_it_is_resolved`.

### Scenario 5: with tabs open, nothing changes (criterion 5)
1. With one tab, then with seventeen (more than fit either width) and the
   first one active, then with a notice as well, show each mode.

Expected: the band is 40 px with its separator; every tab has its cross; the
active tab is revealed; the notice sits under the tabs.
Observed: as expected. The existing tab-strip tests
(`a_tabs_close_control_closes_only_its_own_file_and_keeps_asking`,
`equal_file_names_are_told_apart_in_the_tab_strip_and_long_ones_are_bounded`,
`ctrl_tab_brings_the_active_tab_back_into_view`) and the KTLO-12 header tests
in `header_tests.rs` pass unchanged. New test:
`with_tabs_open_the_strip_and_its_notices_are_as_before`.

### Scenario 6: accessibility and the keyboard (criterion 6)
Expected: the audit passes in the three modes with no file open; Tab from the
app header goes on to the central area, never to the strip.
Observed: as expected. In Code the first stop after the header is the project
files panel's first button (*Refresh*). In Diagram and Split with no file open
the central area holds only hints, so Tab goes from the header to Split's
*Resize code* handle, or back to *Project*; that is unchanged from `main`,
where the empty strip had no control either. Tests:
`the_window_with_no_file_open_names_every_control`,
`tab_from_the_app_header_lands_on_the_central_area`.

Design review of `review_tab_band` (harness captures of the six mock-up views
and the three at 1600 × 1000): the 960 × 540 views are identical to the
mock-up's; nothing is cut off or overlapping.

## Windows

Not run on Windows by hand. CI runs the desktop suite, including the KTLO-24
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`); those show the demo shop with its scenario open,
the state this ticket does not change, not the empty state. See the pull
request's checks. Not accepted for Windows on this report alone.

## Not verified

- Clicking a tab's cross, Ctrl+W and opening a file from the project files
  panel in the release binary: a `--screenshot` cannot click or type. The
  interaction tests cover them; the release captures come from the view state
  the window writes with no tab open.
- The release capture's recovery backup was written by hand and holds no
  draft; the backup in the test holds real drafts written by the session.
- The other notices that bring the band back (an orphan draft, unreadable
  recovery data, the storage label with its spinner, the project file changed,
  invalid or missing) were not raised one by one; `has_notices()` repeats the
  conditions under which `notices()` draws them, and only the dismissable and
  the recovery-backup notices are tested.
- How often the band shows briefly on its own: a storage operation that runs
  longer than 150 ms (saving, or writing the view state a second after a
  change) shows its spinner in the band, so with no file open the band
  appears for that time and the central area moves down and back. This is the
  approved behaviour for "the storage label with its spinner"; how often it
  happens on an ordinary machine was not measured.
- Double scale (960 × 540 at 2×) design review.
- Windows, other than the CI suite and its captures.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb. Item 10 is the
one this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; its output was empty. It was stopped after 90 s, since it does not exit by itself. |
| 2 | Pass: `--folder` opens the project; `after-diagram-one-tab-960x540.png` shows the scenario's tab. |
| 3–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 10 | Pass: at 960 × 540, in Code, Diagram and Split with no file open and in Diagram with one tab (`after-*-960x540.png`), every control is visible. |
