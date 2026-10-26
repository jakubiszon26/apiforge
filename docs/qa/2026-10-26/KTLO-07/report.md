# KTLO-07: Regression smoke list, first Linux run — QA report

- Commit under test: `2c1d98f` (`main` after PR #21, the block library).
- Built with: `cargo build --release -p apiforge-desktop -p apiforge-cli --locked`
  (Rust 1.99.0), in a target directory used by no other branch.
- Host: Linux 7.0.14, Xvfb, Mesa Lavapipe (no GPU), X11 backend. Every run used
  empty scratch `XDG_DATA_HOME` and `XDG_CONFIG_HOME` directories.
- Smoke list: [testing.md, section 3](../../../process/testing.md#3-the-regression-smoke-list).

## Results

| # | Flow | Linux (2026-10-26) | Windows |
| --- | --- | --- | --- |
| 1 | Start with no arguments | **Pass** | owner's pass pending |
| 2 | Open a folder project | **Pass** | owner's pass pending |
| 3 | Add a block from the library panel | **Pass** | owner's pass pending |
| 4 | Wire two blocks | **Pass** | owner's pass pending |
| 5 | Edit source, see the board | **Fail as written**: the board follows in 261–276 ms, not within 250 ms; invalid source holds the board (pass) | owner's pass pending |
| 6 | Run a simulation | **Pass** | owner's pass pending |
| 7 | Save (Ctrl+S) and reopen | **Pass** | owner's pass pending |
| 8 | Close with dirty tabs | **Pass** (see the note on what "close" means) | owner's pass pending |
| 9 | Migrate a legacy workspace | **Pass on the four-project fixture**; six projects not verified | owner's pass pending |
| 10 | Window at 960 × 540 | **Pass** | owner's pass pending |

## How the items were driven

The release binary was driven three ways, and each item says which one it used.

- **Release binary, native window.** `xvfb-run -a -s "-screen 0 1600x1000x24"`
  with `--screenshot OUT.png --folder <project>`, or the binary started on a
  private Xvfb display and captured from the X root window. This host has no
  `xdotool` or XTest, so a small scratch script (Python over `libX11`) sent
  synthetic key presses (`XSendEvent`: Ctrl+E), resized the window
  (`XResizeWindow` to 960 × 540) and closed it (`WM_DELETE_WINDOW`). Pointer
  input cannot be synthesised this way: winit reads the pointer through XInput2.
- **Interaction harness.** Where an item needs the pointer or typing, a scratch
  `egui_kittest` test drove the folder window through named widgets, as
  `folder/library_tests.rs` does. The scratch tests are not committed. They
  prepared the projects in scratch folders, and the release binary then
  reopened those folders, so the result on disk was checked by the binary
  under test. Captures named `…-harness.png` come from the harness: a design
  review aid, not QA evidence by the rule in testing.md.
- **CLI.** `apiforge check` and `apiforge run --folder` from the same build.

## Linux

### 1. Start with no arguments

1. `XDG_DATA_HOME=<empty> timeout 15 xvfb-run -a -s "-screen 0 1600x1000x24" apiforge-desktop`

Expected: the managed project opens; nothing on stderr.
Observed: the window stayed open until the 15 s timeout (exit 124 from
`timeout`, not from the application); stderr was empty. The first start
created `<data>/apiforge/projects/shop` with the demo scenario and three
components, and `recent-projects.json`. A second fresh start, captured from the
X root window after 6 s, shows the shop project in Code with "No problems":
[capture](item1-no-arguments-1600x1000.png).

On a bare `Xvfb` display (and in every `--screenshot` run) Mesa prints two
lines, `libEGL warning: DRI3 error: Could not get DRI3 device` and
`libEGL warning: Ensure your X server supports DRI3 …`. They come from the
graphics driver on a display without DRI3, not from API Forge.

### 2. Open a folder project

1. `apiforge-desktop --screenshot item2.png --folder <copy of the shop project>`: exit 0.
2. Harness: expand `components` in the explorer, click `product.afs`.

Expected: the explorer shows the tree; a scenario opens in Code.
Observed: the explorer lists `Demo shop`, `components`, `scenarios` with the
scenario selected, `.gitattributes`, `.gitignore` and `apiforge.project.json`;
the scenario is open in Code with line numbers and "No problems"
([capture](item2-folder-1600x1000.png)). In the harness, expanding
`components` listed `active-promotion.afs`, `add-to-cart.afs` and
`product.afs`; clicking `product.afs` opened `components/product.afs` in Code
with its editor (`Contents of components/product.afs`).

### 3. Add a block from the library panel

1. Harness: Diagram, click *HTTP request* in the block library, Ctrl+S.
2. Release binary: `--screenshot … --folder <that project>`, and a second start
   with Ctrl+E to Code.

Expected: a card appears on the board and in the source.
Observed: the main board went from 6 to 7 blocks; the new block `request`
(kind Request, card "request (HTTP request)") was selected and the tab dirty;
the source gained `request = GET "/endpoint" { timeout 10s }`. After Ctrl+S the
tab was clean and the file on disk had the statement. The release binary shows
the card on the board ([capture](item3-block-on-board-1600x1000.png)) and the
statement at lines 16–18 of the source ([capture](item3-source-1600x1000.png)).

### 4. Wire two blocks

1. Harness: Diagram; drag *HTTP request* from the library onto the board; drag
   from `Output __order of request` to the top input of *Promotional price*;
   Ctrl+S.
2. Release binary: `--screenshot … --folder <that project>`.

Expected: the wire routes around cards; the source updates.
Observed: the main graph went from 0 to 1 order edge, and the source gained
exactly one line, `@after(request)`, above `promotional_price`. In the release
binary the dashed order wire leaves *request*, runs above *Active promotion*
and *Add to cart* and enters *Promotional price* from the top without crossing
a card ([capture](item4-wire-1600x1000.png)). The block landed beside
*Product* rather than at the intended drop point right of the cards: the
scratch test computed the drop point before the inspector opened and narrowed
the board, so this says nothing about the application.

### 5. Edit source, see the board

1. Harness, Split: select all in the scenario editor and type the same source
   with one added statement, `@label("Smoke total")` / `smoke_total = 2 + 3`;
   step frames and measure until the board's model holds `smoke_total`.
2. Then replace the source with one whose last statement is `smoke_broken = (`
   and whose closing brace is missing.

Expected: the board follows within 250 ms; invalid source holds the board.
Observed:

- The board gained *Smoke total* 261, 263, 265, 265 and 276 ms after the text was typed, in five runs of an optimised (`--release`) test build; 398 and 447 ms in the unoptimised build. The application applies valid source 250 ms after the last
  keystroke by design (`APPLY_DELAY`; ADR 0007, ADR 0012 "Debounce 250 ms") and
  then analyses it on a worker, so the board cannot follow in 250 ms or less:
  the pass condition and the design disagree
  ([capture](item5-board-follows-harness.png)).
- Invalid source: the board kept the same blocks and positions (9 including
  cleanup, *Smoke total* still on it), a banner said "The code has errors. This
  is the last valid version; it is read-only until the code is fixed.", the
  block library was disabled and Problems listed "Expected an expression" and
  "Expected `}`" with *Apply fix*
  ([capture](item5-invalid-holds-harness.png)).

Not driven in the release binary: typing needs keyboard focus in the editor,
which only a click gives, and pointer input could not be synthesised here.

### 6. Run a simulation

1. A TCP listener on `127.0.0.1:4318` (the demo API's address) counting
   connections for the whole item.
2. `apiforge check --folder <shop>`; `apiforge run --folder <shop> --scenario
   scenarios/promotion-and-cart-price.afs`, then the same with `--json`.
3. Harness: *Run* in the window's header.

Expected: the report shows every step; no request is sent.
Observed: `check` exit 0 with two warnings ("The value's type does not match
this input"). Both CLI runs exit 0, "Passed", 18 report lines (the eight
scenario steps and the steps inside the components); the JSON report has
`mode: simulation` and the eight steps `product`, `active_promotion`,
`expected_price`, `add_to_cart`, `promotional_price`, `field_inna_nazwa`
(main) and `delete_cart`, `cart_was_deleted` (cleanup), all passed. From the
window: mode Simulation, status Passed, the same eight steps, none of the
graph's blocks missing; the report panel reads "SIMULATION · Sample responses ·
8 passed" ([capture](item6-simulation-report-harness.png)). The listener saw
**0 connections** during the CLI runs and 0 during the window run.

### 7. Save (Ctrl+S) and reopen

1. Harness: type `//  smoke:  keep   these  spaces<TAB>` and a newline at the
   top of the scenario; in Diagram drag the *Product* card 120 px down; Ctrl+S;
   close the window as the application closes it; open the folder again.
2. Release binary: `--screenshot … --folder <that project>`.

Expected: exact bytes on disk; layout and identities restored.
Observed: after Ctrl+S the tab was clean and the file on disk equalled the
editor buffer byte for byte (1040 bytes; `od -c` shows the doubled spaces and
the tab kept). After reopening, the document had the same ID, the same text,
and every block the same ID, handle and position (*Product* at 40, 346);
reopening did not rewrite `.apiforge/diagram.json`. The release binary shows
*Product* in its moved place ([capture](item7-reopened-layout-1600x1000.png)).

### 8. Close with dirty tabs

1. Harness: type in the scenario, press Ctrl+W.
2. Harness: type in the scenario, close the window as the application does.
3. Harness: type in the scenario, wait 3 s, then end the process without
   closing or dropping the window (`std::process::exit`), standing in for a
   kill. Release binary: open that folder; then close its window with
   `WM_DELETE_WINDOW`.

Expected: Save / Discard / Cancel offered; recovery restores after a kill.
Observed:

- Ctrl+W on the dirty tab opened "Save changes to
  promotion-and-cart-price.afs?" with *Save*, *Discard* and *Cancel*
  ([capture](item8-close-tab-dialog-harness.png)); *Cancel* left the tab open
  and dirty.
- Closing the **window** with a dirty tab asked nothing: the work went to
  `.apiforge/local/recovery.json`, the `.afs` file stayed unchanged, and the
  next start restored the unsaved text as a dirty tab. This is the Stage II
  design ("Closing a dirty tab offers Save, Discard or Cancel. Application
  close may retain dirty tabs through successful recovery without saving",
  `docs/stage-ii-code-workspace.md`). The release binary did the same: after
  `WM_DELETE_WINDOW` it exited 0 within a second, with no dialog, the `.afs`
  checksum unchanged and the recovery file in place.
- After the abandoned process the release binary opened the folder with the
  unsaved line restored, the tab and explorer row marked dirty, *Save* enabled
  and "Unsaved changes" in Problems ([capture](item8-recovered-1600x1000.png)).

The list's wording "Close with dirty tabs: Save / Discard / Cancel offered"
reads as if the window close also asks; it does not, by design. The item is
recorded as a pass against the design.

### 9. Migrate a legacy workspace

1. Copy `fixtures/workspace.json` to `<empty data>/apiforge/workspace.json`;
   record its SHA-256.
2. Start the release binary with no arguments; capture after 10 s.
3. `sha256sum -c`; read `projects.json`; `apiforge check --folder` on each
   migrated project.

Expected: the six-project fixture migrates; originals byte-identical.
Observed: the fixture holds **four** projects ("Sklep demonstracyjny", "System
zgłoszeń", "Bloki operatorów · PoC", "Przepływ: ceny i koszyk"). Migration
activated a catalogue (`projects.json`) with all four (`shop`, `tickets`,
`blocks-poc`, `workflow-poc`) and opened the shop project
([capture](item9-migrated-1600x1000.png)). `workspace.json` was byte-identical
afterwards (`sha256sum -c`: OK). `apiforge check` passed (exit 0) for all four,
with "value's type does not match" warnings in `shop` (2) and `tickets` (1).
The six-project workspace the list names is not in the repository; it is the
owner's real workspace that KTLO-03 (#5) is about.

### 10. Window at 960 × 540

1. Release binary on a 1600 × 1000 display, window resized to 960 × 540 with
   `XResizeWindow`; the X root outside 960 × 540 was black, so the window
   really had that size. Code captured, Ctrl+E to Diagram; Split restored from
   the project's saved mode (`.apiforge/local/workspace.json`).
2. Harness at 960 × 540 in each mode: header controls inside the window and
   disjoint; a click on *HTTP request* in Diagram.

Expected: all controls visible and usable in Code, Diagram and Split.
Observed: in all three modes the header (Project, Code, Diagram, Split, Save,
Save all, Run options, Run) is whole and separate. Code shows explorer, editor
and Problems ([capture](item10-code-960x540.png)); Diagram shows the block
library, the board at 41 % with the minimap, *Add step* and the zoom bar
([capture](item10-diagram-960x540.png)); Split shows code, Problems and the
board at 32 % with *Add step*, and no library, as KTLO-05b specifies below
1440 px ([capture](item10-split-960x540.png)). The harness found every header
control inside the window and disjoint in each mode, and the library click
added a block in Diagram at 960 × 540. Seen but not a failure: at 960 × 540 the
library shows four blocks before it scrolls, and card titles at 32–41 % are
small.

## Windows

Not run by the agent. Every item is "owner's pass pending": the owner's local
build, or the CI artifacts of a branch where a capture is enough.

## Not verified

- **Windows**: every item.
- **Item 9 on six projects**: the six-project workspace is not in the
  repository (KTLO-03, #5); only the four-project `fixtures/workspace.json`
  was migrated.
- **The release binary's own pointer and text input** (items 2–5, 7, 8): the
  editing steps ran in the interaction harness; the release binary opened and
  captured the results (items 3, 4, 7, 8) or was not used (item 5). Typing,
  clicking and dragging in the native window were not exercised.
- **Item 8, a real kill**: the dirty state came from the harness, whose process
  ended without closing the window; the release binary was not itself killed
  while dirty.
- **Item 6 from the native window**: the window's *Run* was pressed in the
  harness; the release binary ran the simulation only through the CLI.
- **Item 5 timing in the native window**: measured in the harness (see the
  item for the build profile).
- **Item 10 under a window manager**: the window was resized without one;
  decorations and a user's drag-resize were not tried.
- Screen reader, HiDPI scaling on the native host, Wayland.

## Notes for follow-up tickets

- **Item 5 (the list, not the application)**: "Board follows within 250 ms"
  contradicts the documented 250 ms debounce after the last keystroke
  (ADR 0007, ADR 0012); measured 261–276 ms in an optimised build. Either the pass condition becomes a time
  after the typing pause, or the debounce changes, which is the owner's call.
- **Item 8 (the list's wording)**: the window close keeps dirty tabs through
  recovery without a dialog, by design; only closing a tab asks.
- **Item 9 (the list's wording)**: the list names a six-project fixture that
  does not exist; `fixtures/workspace.json` has four projects.
- **Shared Cargo target directory**: building this branch in a target
  directory that other worktrees also build in produced a test binary whose
  `apiforge-storage` came from another worktree's sources
  (`missing field library in PanelSizes`). Cargo's artefact hashes for
  workspace members do not include the checkout's path, so parallel worktrees
  overwrite each other's artefacts. Everything in this report comes from a
  separate target directory.
