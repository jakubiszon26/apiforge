# GUI testing

This procedure implements [ADR 0013](decisions/0013-interface-testing.md) for
`apiforge-desktop`. It is the same for every contributor and coding agent, in a
desktop session or in a terminal without a window system.

| Layer | Tool | Display | Purpose |
| --- | --- | --- | --- |
| Model tests | `Context::run_ui` | none | Editing, storage and state rules |
| Interaction tests | `egui_kittest` | none | Behaviour and measurable layout through named widgets |
| Design review | `egui_kittest` captures | none | Visual quality of the views a change touches |
| Native probes | release binary | virtual X11 | Window system, renderer, capture and exit |
| Exploration | `egui_mcp` | none, or virtual X11 | Reproducing and investigating problems |

Pixel comparison against committed reference images is not used; see decision 8
of the ADR.

## Requirements

Interaction tests need only the Rust toolchain. Design review captures and
headless screenshots need a Vulkan driver; on a machine without a GPU install
Lavapipe (`mesa-vulkan-drivers` on Debian and Ubuntu). The native host also
needs Xvfb.

## Interaction tests

Interaction tests are the default for every change to the interface.

- They live in `#[cfg(test)]` modules of `apiforge-desktop`, next to the code
  they test. [`folder/interaction_tests.rs`](../crates/apiforge-desktop/src/folder/interaction_tests.rs)
  is the reference example.
- Harnesses come from [`crate::testing`](../crates/apiforge-desktop/src/testing.rs):
  `folder_harness(root, size)` opens a folder window and waits until opening has
  finished; `workspace_harness(size)` opens the workspace window with the demo
  workspace. `shop_project()` prepares the demo shop folder project in a
  temporary directory. `MINIMUM` (960 × 540) and `WIDE` (1600 × 1000) are the
  standard window sizes.
- An application constructed for a test receives the harness's own
  `egui::Context`. A context created separately does not receive the harness's
  input.
- Widgets are found by accessible label or role: `get_by_label`,
  `get_by_role_and_label`, `query_by_label_contains`. A label that appears
  more than once cannot be used with `get_by_label`; use a role or a unique
  label. A position is used only relative to a named widget's `rect()`.
- After an action, call `settle(&mut harness)`. It steps until no storage work
  is in flight and the interface stops requesting immediate repaints. Fixed
  sleeps are not allowed.
- Assertions check application state and files first, then the
  accessibility tree.
- Measurable layout rules use `assert_within_window(&harness, labels)` and
  `assert_disjoint(&harness, labels)`, at `MINIMUM` and at `WIDE`.
- A widget a test needs but cannot find by name is a defect in the interface.
  Give it an accessible name in the same change.
- Tests prepare projects from fixtures in temporary directories. Run the suite
  with isolated configuration and data directories:

```bash
XDG_CONFIG_HOME=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d) \
  cargo test -p apiforge-desktop --locked
```

## Design review

A change that alters appearance — layout, styling, new or moved widgets, or
text that may not fit — finishes with a design review of the views it touches.

1. **Views.** Review views are ignored tests whose names start with `review_`.
   The standard set in `testing::review` renders each folder mode at 960 × 540,
   at 1600 × 1000 and at 960 × 540 with double scale, and the workspace window.
   A change to a view that is not covered adds a review test for it, using
   `capture(&mut harness, name, region)`. The list is kept: it is the candidate
   set for snapshot comparison.
2. **Render.** `tools/gui/review.sh [filter]` renders the views and prints the
   path of a gallery page. Captures are written to `target/gui-captures/`
   (`APIFORGE_CAPTURE_DIR` overrides it) and are never committed.
3. **Inspect.** Look only at the captures of the touched views. Prefer a
   `region` around the named widgets concerned over a whole window; a whole
   window is for judging the overall layout.
4. **Checklist.**
   - Alignment and spacing are consistent with neighbouring controls and the
     theme in `theme.rs` and `widgets.rs`.
   - No widget, card, port or wire overlaps another; nothing needed is cut off.
   - Text that identifies something is not truncated where space allows; where
     it is truncated, the full text is available on hover.
   - States (selected, dirty, error, disabled) are distinguishable, and
     keyboard focus is visible.
   - The minimum window remains usable, and double scale is sharp and keeps
     proportions.
5. **Outcome.** A finding that can be measured becomes an assertion in an
   interaction test. A finding that cannot is fixed in the change or reported
   in the issue tracker with its capture attached.

## Native probes

The `--smoke-test` and `--screenshot` commands described in
[development.md](development.md) check what interaction tests cannot: winit,
the presentation surface, the graphics driver, capture and process exit.

## Exploration with egui_mcp

Exploration is for reproducing a reported problem, investigating behaviour and
checking a change in a running interface. Its results are tests, not reports.

### Setup

```bash
tools/gui/egui-mcp.sh --install
```

installs the pinned `egui_mcp` release outside the repository. The project's
[`.mcp.json`](../.mcp.json) registers `tools/gui/egui-mcp.sh` as the MCP
server `egui`; it reports when the release is not installed.

### Hosts

Start a host in the background; it prints `HOST READY <address>` and runs until
it is stopped. Configuration and data are isolated in a temporary directory.

```bash
tools/gui/host.sh headless
```

is the default: the folder window with the demo shop project in a test
harness, serving `egui_inspection` on a free loopback port. It needs no
display. `APIFORGE_HOST_FOLDER` opens another folder project instead.

```bash
tools/gui/host.sh native [application arguments]
```

builds the application with the `inspection` feature and runs it on a private
Xvfb display with the X11 backend and Lavapipe. Without arguments it opens
`fixtures/workspace.json`. Use it only when the problem involves the window
system, the clipboard, native dialogs or the real renderer.

Both hosts bind loopback only. Never bind another address: the inspection port
has no authentication.

### Rules

1. `attach` with the host and port from the ready line.
2. Read the tree with filtered `query_tree` calls (`role`, `label_contains`,
   `value_contains`). Do not request the whole tree.
3. Act by label, for example `click` with `label_contains: "Split"`, then
   `wait_for` the widget the action should produce.
4. Check the result through the tree and through files on disk. Take a
   screenshot when the question is visual; first `wait_for` with `min_steps` so
   animations finish.
5. Reproduce a defect as a failing interaction test before changing the code,
   then make it pass. A defect that cannot be expressed as a test yet is
   reported in the issue tracker with steps that use widget labels.
6. Stop the host when finished.

## Versions

`egui_kittest`, `egui_inspection` and `eframe` use the same exact version.
`egui_mcp` is the release built for the same egui minor version; tool names
differ between its releases, and this document names those of the pinned
release. An egui upgrade updates all of them, `tools/gui/egui-mcp.sh` and this
document in one change.
