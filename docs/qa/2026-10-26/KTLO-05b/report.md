# KTLO-05b: Block library beside the board (PR A) — QA report

- Branch and commit: `feat/ktlo-05b-block-library` (see the PR head commit)
- Built with: `cargo build --release -p apiforge-desktop --locked` (clean)
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, the desktop test suite (327 passed,
  16 ignored) and `apiforge-convert-v1 --fixtures --check` all pass.
- Compared against the approved mock-up: `docs/tickets/KTLO-05a.md` (option A).

## Scope of this PR

This is **PR A**: the panel and its behaviour. Criterion 6 (persist the
panel width in the project workspace state) and criterion 5's second half
(the *Add step* popup's Variables and Operators sections, shown in Split below
1440 px) are follow-ups; see `docs/tickets/KTLO-05b.md`. Criterion 6 needs an
owner decision first — it changes the persisted `PanelSizes`, whose
`deny_unknown_fields` means an older build would reject a newer workspace file
(raised on issue #8).

## A note on evidence

Release-binary captures at 960 × 540 and 1600 × 1000 in a chosen mode need
`--screenshot --mode --size`, which KTLO-08 (PR #4) adds; it is not merged, so
the current release binary cannot be driven to Diagram/Split at a given size.
The captures below are therefore from the design-review harness (the real egui
interface rendered on the native host, `tools/gui/review.sh`). Per
`docs/process/testing.md` these are design-review aids; the **behaviour** is
proven by the acceptance tests in `folder/library_tests.rs`, which drive the
real folder application through named widgets. Release-binary captures are
deferred until PR #4 lands.

## Linux

Each scenario corresponds to an acceptance test (all passing).

### 1. The panel and its sections (criterion 1)
Diagram shows a panel named *Block library* with search, the Blocks /
Variables / Operators sections and *Project components*; the *Examples* entry
is left out (KTLO-05a decision 2). Every control has an accessible name and
`stage_ii_screens_and_dialogs_name_every_control` passes with the panel on
screen.
Test: `diagram_shows_a_named_block_library_panel_with_its_sections`.
Captures: [before](before-diagram-1600x1000.png) · [after](after-diagram-1600x1000.png) (1600 × 1000).

### 2. Click a block (criteria 2, 3)
Clicking a block adds it at the board's visible centre, selected, and dirties
the tab; the source gains its statement. Drag adds at the drop point through
the existing `Command::AddNode` path in `canvas/mod.rs` (unchanged here).
Expected: one node added at the centre, selected, tab dirty.
Observed: as expected (verified on an empty board so the centre is free and
the placement is exact).
Test: `clicking_a_block_adds_it_at_the_visible_centre_selected_and_dirty`.

### 3. Operators and variables (criterion 4)
An operator chip adds a Value block with that operator; *Variables* lists the
document's variable nodes and *Add variable* adds one.
Tests: `an_operator_chip_adds_a_value_block`, `add_variable_adds_a_variable_node`.

### 4. Split threshold (criterion 5, panel half)
Split shows the panel at 1440 px and wider and not below it, where the *Add
step* popup takes over (popup sections are PR B).
Observed: the panel appears in wide Split and is absent in narrow Split — the
narrow-Split capture is unchanged from `main`.
Test: `split_shows_the_library_only_when_wide_enough`.
Captures: [before](before-split-1600x1000.png) · [after](after-split-1600x1000.png) (1600 × 1000, code · library · board).

### 5. Read-only board (criterion 7)
When the board is read-only (invalid or held source) the whole panel is
disabled, with no separate explanation; the board's banner already says why.
Observed: `library_panel` disables the panel content when `!snapshot.editable`
and `library_action` ignores actions then. Not covered by a dedicated
interaction test (see Not verified).

### 6. Minimum window (criterion 8)
At 960 × 540 in Diagram the panel's controls and the board are inside the
window.
Test: `the_library_fits_the_minimum_window_in_diagram`.
Captures: [after, Diagram 960 × 540](after-diagram-960x540.png).

### 7. Empty-board hint (criterion 9)
The empty board's hint reads "Drag a block from the library or add the first
step." only when the library is in view, otherwise "Add the first step."
Test: `the_empty_board_hint_depends_on_the_library_being_in_view`.

## Windows

Not run. Windows evidence comes from the CI artifacts of this PR's run; folder
screenshot artifacts depend on KTLO-08 (PR #4). The owner's local Windows pass
is pending.

## Not verified

- **Criterion 6 — panel-width persistence.** Deferred; it changes the
  persisted `PanelSizes` (an older build rejects a newer workspace file under
  `deny_unknown_fields`). Raised on issue #8 for an owner decision; within a
  session egui still remembers the resized width.
- **Criterion 5 — popup Variables/Operators.** PR B.
- **Criterion 3 — drag placement** is not re-tested here; it reuses the
  existing, already-tested canvas drop path. Only click placement is asserted.
- **Criterion 7 — read-only** is implemented but has no dedicated interaction
  test (setting a last-valid/held board up through named widgets is involved);
  verify by review of `library_panel`/`library_action` and by capture.
- **Release-binary captures** at the two sizes/modes pending PR #4.
- **Windows** and **screen-reader** sessions not run.

## Regression smoke list (Linux, 2026-10-26)

Only item 3 is exercised by this change; the full list is KTLO-07.

| # | Flow | Result |
| --- | --- | --- |
| 3 | Add a block from the library panel | **Pass** — the library panel is present; clicking a block adds a card to the board and its statement to the source (`clicking_a_block_adds_it_at_the_visible_centre_selected_and_dirty`). This item failed on `main`; it passes on this branch. |

The other items are run for KTLO-07 on the release binary.
