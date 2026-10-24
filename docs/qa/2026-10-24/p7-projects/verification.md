# Stage II P7a: Open Folder, recent projects and switching

Branch: `feat/stage-ii-p7-projects`. Base: unmerged P8 `2939545`
(`docs/stage-ii-p8-consistency`), following the reviewed package ownership and P6d's
run/close/export integration.

The Project menu offers Open Folder and up to 32 recent projects, with full paths
on hover. Recent metadata is separate from project definitions in the application's
data directory. It retains identity, order and active path, bounds reads/writes,
preserves unsupported/unreadable files, and retains the in-memory list even when
it cannot persist. Activating the current folder does not reopen it read-only.

A destination is checked without replacing the active view or recording a recent
activation. Switching first cancels an active run and waits for cleanup, then
writes the old project's recovery and view state. Only success installs the new
session and records its activation. Old field-widget and text-history state is
retired after recovery succeeds, including when a copied project shares IDs.

Failed recovery or dirty settings retain the current project and offer Keep,
Retry, Export or Discard with explicit switching labels. Export must succeed for
the current work before switching; it cannot authorise closing newer edits.
Selecting a folder without a manifest writes no project or local metadata and
shows the existing explicit Initialise flow. A bad destination leaves the current
project usable.

Six new regressions cover recent metadata identity/order/restart and future-file
preservation, pointer-driven return through the recent menu with exact dirty
source/field recovery, failed recovery retaining the active project/recent path,
uninitialised/bad destinations and read-only export before switching.

Native Linux/Xvfb/Mesa review used XTest pointer and keyboard input:

- [Project menu at 960 × 540](project-menu-960.png).
- [Switched from dirty Alpha to Beta](switched-beta-960.png).
- [Returned to Alpha with the exact invalid edit restored](recovered-alpha-960.png).

The native file-portal chooser itself remains part of P7f's environment/native
review. Ordinary opening, recent metadata and recovery still use synchronous I/O
until P7d's storage worker; this package does not activate normal folder startup
or migration. Windows QA and directory durability remain open release gates.

Release layout budgets were also checked after the header layout change: layered
layout 0.33 ms / 20 ms; full routing 24.31 ms / 30 ms; dragging 0.20 ms / 8 ms;
smoothing 0.26 ms / 8 ms; placing eight new cards 0.06 ms / 8 ms. The random-wiring
stress case has no budget (791 ms). These are measurements on this Linux host.

Verification on Linux (2026-10-24), Rust 1.99.0: formatting, all-target workspace
clippy with warnings denied, converter fixture checks and serial all-target
workspace tests passed: **849 passed, two parent-launched helpers ignored**.
Config/data directories were isolated. The UI quality gate passed **10/10**, with
`CARGO_TARGET_DIR=/tmp/apiforge-gate`. All release layout benchmarks with budgets
passed. No language implementation changed.
