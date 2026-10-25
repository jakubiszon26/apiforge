# ADR 0013: Interface testing through the accessibility tree

- Status: accepted
- Date: 2026-10-25
- Scope: `apiforge-desktop` tests and development tooling; no change to the
  application's behaviour or release builds
- Procedure: [GUI testing](../gui-testing.md)

## Context

The desktop interface is tested in two ways. Unit tests call
`Context::run_ui` with synthesised events and inspect application state; tests
that need a widget's position walk the painted shapes to find its text. Native
behaviour is checked by opening the release binary under a virtual display and
by driving a real window with operating-system input tools, using screenshots
as the result.

Neither form names what it interacts with. Shape walking breaks when drawing
changes, operating-system input needs a display server and extra system tools,
and a screenshot cannot be asserted on directly. Coding agents working on the
interface need a procedure that is the same on every machine, including a
terminal session without a window system, and that produces tests rather than
one-off observations. Visual quality still needs to be seen: overlap,
alignment, spacing and truncation are not all visible in state or in widget
names.

egui builds an AccessKit tree of named widgets every frame. Two tools use it:

- `egui_kittest` runs an egui interface inside a test through a `Harness`,
  finds widgets by role and label, injects input and steps frames. With its
  `wgpu` feature it renders frames without a display handle, preferring a CPU
  adapter, through the same renderer and fonts as the application.
- `egui_mcp` is a Model Context Protocol server that drives an egui
  application over the `egui_inspection` protocol: it reads the widget tree,
  clicks, types, scrolls, waits for widgets and takes screenshots. eframe's
  `inspection` feature serves that protocol when `EGUI_INSPECTION` is set.
  `egui_inspection` is an `egui::Plugin`, and the kittest `Harness` fulfils the
  screenshot and resize requests the plugin issues, so a harness can serve the
  protocol without any window system.

## Decision

1. **Interaction tests use `egui_kittest`.** New and changed interface
   behaviour is tested by finding widgets through the accessibility tree and
   asserting on application state, on files and on the tree. The existing
   `run_ui` tests remain for model-level behaviour.
2. **Widgets that tests or agents need have accessible names.** A missing name
   is fixed in the interface, not worked around with coordinates.
3. **Layout rules that can be measured are assertions.** Widgets that must be
   visible are inside the window, and widgets that must not overlap are
   disjoint, checked on the rectangles of named widgets.
4. **Visual quality is checked by a design review.** A change that alters
   appearance renders a fixed set of review views through kittest and inspects
   them against a checklist. The views are ignored tests, so the list is kept in
   code and grows with the interface. Captures are written to the build
   directory and are never committed. A finding that can be measured becomes an
   assertion under decision 3.
5. **Agents explore the interface with `egui_mcp`.** The default target is a
   headless host: an ignored test that builds the application in a kittest
   harness and serves `egui_inspection` on a free loopback port. A native window
   built with the `inspection` feature under a virtual display is used only
   when a problem depends on the window system, the clipboard, native dialogs
   or the real renderer.
6. **Every reproduced defect becomes a failing interaction test before it is
   fixed.** An exploration session is never the regression test.
7. **Versions are pinned and move together.** `egui_kittest` and
   `egui_inspection` are exact dev-dependencies matching `eframe`, and the
   `egui_mcp` release is the one built for the same egui minor version. An egui
   upgrade updates all of them in one change.
8. **Visual snapshot comparison is deferred.** Rendered pixels depend on the
   driver and its version. Reference images are not committed until the project
   has a pinned reference environment in which they are generated and compared;
   the review views of decision 4 are then the candidate snapshot set.
9. **The `inspection` feature is off by default** and is never enabled in
   release builds. The inspection port is bound to loopback only; it has no
   authentication.

## Alternatives considered

- **Shape walking over `run_ui` output.** Kept for existing tests, not extended:
  it identifies widgets by drawn text and position, not by name or role.
- **Operating-system input under a virtual display with screenshot text
  recognition.** Needs a display server and input tools on every machine, waits
  on real time, and its oracle is recognised text rather than state.
- **AT-SPI based automation.** Linux only, requires an accessibility bus and a
  client embedded in the application.
- **Pixel snapshots as the main oracle.** Renderer-dependent; see decision 8.
- **No visual inspection.** Leaves alignment, spacing and visual overlap
  unchecked; see decision 4.

## Consequences

- `apiforge-desktop` gains dev-dependencies on `egui_kittest` (`wgpu`) and
  `egui_inspection` (`plugin`) and a crate feature `inspection` that enables
  `eframe/inspection`. Application code and release artefacts are unchanged.
- Application constructors receive the harness's `egui::Context`, as they do
  eframe's. Harnesses wait for storage workers before tests interact.
- Interaction tests run with `cargo test` on any machine; they need no display
  and no GPU. Review captures and headless screenshots need a Vulkan driver;
  without a GPU that is Lavapipe.
- Interaction tests do not exercise winit, the presentation surface, native
  dialogs or process lifetime. The native probes described in
  [development.md](../development.md) remain the check for those.
- Accessible names become part of the interface contract, which also serves
  assistive technology.
- `egui_mcp` is a young project whose tool names have changed between minor
  releases. Only agent tooling depends on it; tests and builds do not.
