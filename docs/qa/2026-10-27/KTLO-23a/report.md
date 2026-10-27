# KTLO-23a: A component runs from its tab — QA report

- Branch: `feat/ktlo-23a-component-run`; captures from the release binary of
  `bdc037c` (the implementation and its test-helper correction, on top of
  `main` at `280d6bd`, KTLO-18 and KTLO-22 included)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: [docs/tickets/KTLO-23.md](../../../tickets/KTLO-23.md)
  (`../KTLO-23/mock-*.png`), approved as a whole (decision 5); header width
  decision 6 = 650 px on every tab
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked --no-fail-fast` (isolated `XDG_CONFIG_HOME` and
  `XDG_DATA_HOME`; 1055 passed, 0 failed) and `apiforge-convert-v1 --fixtures
  --check`: all pass. The 18 KTLO-23a tests (11 desktop, 6 runner, 1 project)
  passed ten runs in a row.

## Captures

`after-*.png` are from the **release binary** of this branch, started with
`--managed` on an empty data directory (which created the demo shop) under
Xvfb with Lavapipe and no window manager. Every state was reached by
**clicking**: a script sent real pointer events through the X server's XTEST
extension (the explorer's `components` folder, `product.afs`, *Diagram*,
*Run*, the *Product* step row, …) and captured the window with the X
`GetImage` request; `--screenshot` was not used, because it cannot click.
The window opened at 1600 × 1000; for 960 × 540 it was resized through the X
server, so the board's zoom and the panel's height at 960 × 540 are what the
window kept from 1600 × 1000, as after a manual resize. The run is a
simulation of the demo's `Product` component (its only input, `id`, defaults
to 123), not the mock-up's `add-to-cart`, whose `product` input has no
default and fails under this ticket by design (criterion 7).

| View | After (release binary) | Approved mock-up |
| --- | --- | --- |
| Header, component tab, Diagram, 1600 × 1000 | [after](after-header-component-diagram-1600x1000.png) | `mock-header-component-1600x1000.png` |
| Header, component tab, Split, 1600 × 1000 | [after](after-header-component-split-1600x1000.png) | — |
| Header, component tab, Diagram / Code / Split, 960 × 540, after a run | [Diagram](after-header-component-diagram-960x540.png), [Code](after-header-component-code-960x540.png), [Split](after-header-component-split-960x540.png) | `mock-header-component-960x540.png` |
| *Run*'s hover text, 960 × 540 | [after](after-run-hover-960x540.png) | — |
| Run results, Diagram, 1600 × 1000, export step selected | [after](after-run-results-component-1600x1000.png) | `mock-run-results-component-1600x1000.png` |
| Run results, Diagram, 1600 × 1000, request step selected | [after](after-run-results-component-request-1600x1000.png) | — |
| A scenario run afterwards, Split, 960 × 540 | [after](after-scenario-run-960x540.png) | — |

Differences from the mock-up, all expected: the component is `Product` (two
steps) instead of `add-to-cart` (five), so the panel is at its default height
and every step is in view; the durations and the value are computed, not
staged; the export step shows its value with no *Check* or *To variable*
(criterion 11; the mock-up still had them); the "Running saved code." notice
under the tab strip, which the mock-up did not draw, is the window's usual
notice after a run.

## Linux

### Scenario 1: *Run* and *Run options* on a component tab (criterion 1, decision 6)
1. Open the demo shop, expand `components` in the explorer, open `product.afs`.
2. Look at the header in Diagram, Code and Split at 1600 × 1000 and 960 × 540;
   hover *Run*.

Expected: *Run options* and *Run* to the right of *Publish*, every control
inside the window; the hover text reads "Simulate the selected scenario or
component, including unsaved valid code".
Observed: as expected and as in the mock-up. At 960 × 540 the toolbar from
*Code* to *Run* fits with "Demo shop" untruncated. The mode tabs sit about
70 px further left on a component tab than on a scenario tab, because the
toolbar is wider (as in the mock-up); the title keeps its width. The hover
text is as expected. Tests:
`ktlo23_a_component_tab_holds_run_and_run_options_beside_publish` (every
mode, both sizes, `assert_within_window` and `assert_disjoint`, the hover
text); `long_project_names_do_not_overlap_header_actions_in_writable_or_read_only_windows`
now opens a component tab too, in the writable and the read-only window, and
checks that the long title clears *Code* by 8 px and has one width on both
tabs.

### Scenario 2: a component with defaults runs (criteria 2 and 6)
1. With `product.afs` active in Diagram, click *Run*.
2. Click the *Product* row, then the *Fetch product* row.

Expected: *Run results* opens, PASSED, one row per block; the export's value
under *Outputs*; the chosen step's block selected on the component's board
with its inspector open; no error-policy box.
Observed: as expected. PASSED, "2 passed", rows *Fetch product* and
*Product*; the export's *Outputs 1* is `value : Object (4)` with the
product's sample; the *Product* export block is selected on the board and
its inspector (EXPORT · Product) is open; no *Continue remaining steps* box.
Tests: `ktlo23_run_on_a_component_lists_its_steps_and_the_exported_value`
(also: the file on disk is unchanged),
`ktlo23_choosing_a_step_selects_its_block_on_the_components_board` (both
sizes); runner: `ktlo23_a_component_runs_on_its_own_with_its_input_defaults`.

### Scenario 3: no *Check* or *To variable* on the export (criterion 11)
Expected: the export step's value offers neither, nor the "Choose a field →
add a check" hint; a request step still does.
Observed: as expected in the release binary (compare the two 1600 × 1000
run-results captures). Test:
`ktlo23_a_components_export_step_offers_no_check_or_variable`. This is the
agent's technical decision, reported in the ticket for the owner to
overrule.

### Scenario 4: unsaved text, staleness, last valid version (criteria 3 and 5)
Not tried in the release binary. In the harness: changing the default 123 to
124 without saving runs `/products/124` and leaves the file as saved; editing
again marks the report "Graph changed · results from an earlier version";
invalid code shows *Run the last valid version?* with "Component: Product",
runs nothing until *Run this version*, then runs. Tests:
`ktlo23_the_run_uses_the_unsaved_text_and_a_later_edit_makes_it_stale`,
`ktlo23_invalid_component_code_offers_the_last_valid_version`; project:
`ktlo23_a_component_prepares_a_run_of_its_working_text_and_its_pinned_calls`.

### Scenario 5: a component calling another (criterion 4)
Not tried in the release binary (the demo has no such component). In the
harness, a new component calling `product@1()` runs revision 1 even while
`product.afs` holds an unsaved 999, and its inner steps are listed indented
under the call. Tests:
`ktlo23_a_called_component_runs_its_pinned_revision_indented`; runner:
`ktlo23_a_called_component_runs_its_pinned_revision`.

### Scenario 6: an input without a default (criterion 7)
Not tried in the release binary. In the harness, *Run* on `add-to-cart.afs`
fails: the *Product ID* step reports "Missing required input: product." and
the steps after it do not run. Tests:
`ktlo23_an_input_without_a_default_fails_its_step_as_missing` (desktop and
runner).

### Scenario 7: live requests (criterion 8)
Not tried in the release binary (no demo API was started). In the harness,
against a loopback server: *Run options → Run live requests* on
`product.afs` sends `GET /products/123` and passes; a second live run held
in flight writes the run to `.apiforge/local/journal.json` under "Product",
and the empty report reads "Results will appear while the component runs.";
those journal bytes, restored after closing as a killed process would leave
them, are listed at the next start ("A live run did not finish", "Product ·
…"). A component run has no cleanup phase, so its journal records none.
Tests: `ktlo23_live_requests_from_a_component_are_sent_and_journalled_under_its_name`;
runner: `ktlo23_a_live_component_run_is_journalled_under_its_name`.

### Scenario 8: scenario runs unchanged (criterion 10)
Observed: in the same release session, *Run* on the demo scenario reported
PASSED, 8 passed, with the *Continue remaining steps* box and "Choose a field
→ add a check" as before ([capture](after-scenario-run-960x540.png)). Every
existing run test in `folder/tests.rs` and `folder/acceptance_tests.rs`
passes unchanged.

### Accessibility
`a_component_tab_with_run_results_names_every_control` audits a component's
tab with results in Diagram, Split and Code at both sizes. It found two older
unnamed controls, now named, with no visible change: the run results panel's
resize edge (*Resize run results*) and the rows of the Problems list (named
by their text; the demo's `product.afs` carries a warning, "The value's type
does not match this input", which no earlier audit showed).

## Windows

Not run on Windows by hand. CI runs the whole suite, including the KTLO-23a
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`), which show the scenario tab and no component
tab or run; see the pull request's checks. Not accepted for Windows on this
report alone.

## Not verified

- In the release binary: criteria 3, 4, 5, 7 and 8 (unsaved text, a nested
  component, last valid version, a missing input, live requests); the tests
  cover them in the harness.
- A live component run against the real demo API (`apiforge-demo-api`) and a
  real process kill during a live run; the test restores the journal bytes a
  killed process would leave.
- The read-only window and a long project name in the release binary (the
  header test covers both).
- Double scale (960 × 540 at 2×) design review.
- Windows, other than the CI suite and the standard captures.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb. Items 6 and 10
are the ones this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr stayed empty for the whole session. |
| 2 | Pass: the explorer showed the tree; the scenario opened in Code, `product.afs` from the explorer. |
| 3–5, 7–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 6 | Pass: *Run* (simulation) on the scenario reported every step, PASSED, 8 passed; on `product.afs`, PASSED, 2 passed. |
| 10 | Pass: at 960 × 540 in Diagram, Code and Split, with a component tab and its run results, the header's controls are visible (`after-header-component-*-960x540.png`). |
