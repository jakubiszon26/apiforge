# KTLO-23b: A component run asks for the inputs it lacks — QA report

- Branch: `feat/ktlo-23b-run-inputs`; captures from the release binary of
  `e051fef` (the implementation on top of `main` at `dc13f71`, KTLO-23a
  included)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: [docs/tickets/KTLO-23.md](../../../tickets/KTLO-23.md)
  (`../KTLO-23/mock-run-inputs-960x540.png`, `mock-run-inputs-1600x1000.png`,
  `mock-run-inputs-1600x1000-filled.png`), approved as a whole (decision 5);
  the dialog appears only when an input has no default (decision 1)
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test --workspace
  --all-targets --locked --no-fail-fast` (isolated `XDG_CONFIG_HOME` and
  `XDG_DATA_HOME`; 1078 passed, 0 failed) and `apiforge-convert-v1 --fixtures
  --check`: all pass. The 7 KTLO-23b tests (4 acceptance, the audit, 2 unit
  tests of the dialog) passed ten runs in a row.

## Captures

`after-*.png` are from the **release binary** of this branch, started with
`--managed` on an empty data directory (which created the demo shop) under
Xvfb with Lavapipe and no window manager. Every state was reached by
**clicking and typing**: a script sent real pointer and key events through
the X server's XTEST extension (the explorer's `components` folder,
`add-to-cart.afs`, *Diagram*, *Run*, the dialog's fields, *Cancel*, the
dialog's *Run*, …) and captured the window with the X `GetImage` request;
`--screenshot` was not used, because it cannot click. The window opened at
1600 × 1000 and was resized through the X server to 960 × 540 and back.

| View | After (release binary) | Approved mock-up |
| --- | --- | --- |
| The dialog as it opens, 1600 × 1000 | [after](after-run-inputs-1600x1000.png) | `mock-run-inputs-1600x1000.png` |
| The dialog as it opens, 960 × 540 | [after](after-run-inputs-960x540.png) | `mock-run-inputs-960x540.png` |
| Filled, *Run* enabled, 960 × 540 | [after](after-run-inputs-960x540-filled.png) | — |
| Filled, *Run* enabled, 1600 × 1000 | [after](after-run-inputs-1600x1000-filled.png) | `mock-run-inputs-1600x1000-filled.png` |
| The run with the entered values, *Cart item* selected | [after](after-run-results-entered-values-1600x1000.png) | — |
| *Run* again: the values kept | [after](after-run-inputs-kept-1600x1000.png) | — |
| `product.afs` (all defaults) runs with no dialog | [after](after-product-runs-with-no-dialog-1600x1000.png) | — |
| The scenario runs as before | [after](after-scenario-run-1600x1000.png) | — |

Differences from the mock-up: the field that has the keyboard focus is
outlined in the accent colour, with the caret in it (the dialog focuses the
first empty field when it opens; the mock-up drew none focused); the board
behind the dialog is at the zoom the window chose (76 % and 34 %), not the
mock-up's 75 % and 33 %; the header and tab strip are today's, 8 px higher
than in the mock-up's frame. The dialog itself — eyebrow, title, hint, field
titles, type and contract lines, note, *Cancel* and the disabled or enabled
*Run* — matches the mock-up at both sizes.

## Linux

### Scenario 1: an input without a default asks first (criterion 9, decision 1)
1. Open the demo shop, expand `components`, open `add-to-cart.afs`, switch to
   Diagram.
2. Click *Run* in the header, at 1600 × 1000 and at 960 × 540.

Expected: the *Run inputs* dialog, "COMPONENT · ADD TO CART"; `product`
("Object · Product contract · no default, enter a value to run") empty;
*Quantity · quantity* ("Number · pre-filled from its default") holding 1;
*Run* disabled; nothing runs.
Observed: as expected and as in the mock-up, at both sizes; no *Run
results* panel opened. Test:
`ktlo23b_an_input_without_a_default_asks_for_the_run_inputs` (both sizes:
the fields by their accessible names "product · Object" and "Quantity ·
quantity · Number", their values, `assert_within_window` and
`assert_disjoint`, *Run* disabled while `product` is empty or not JSON and
while `quantity` is empty or not a number).

### Scenario 2: *Cancel* runs nothing (criterion 9)
1. At 960 × 540, type `{ "id": 123 }` into `product`, set `quantity` to 2
   ([capture](after-run-inputs-960x540-filled.png): *Run* enabled).
2. Click *Cancel*, then *Run* in the header again.

Expected: no run; the dialog opens again as it did the first time.
Observed: as expected: no *Run results* panel, and the reopened dialog was
pixel-identical to [the first one](after-run-inputs-960x540.png) (`product`
empty, `quantity` 1): what was typed before *Cancel* is not kept. Test:
`ktlo23b_cancel_runs_nothing` (also Escape).

### Scenario 3: the dialog's *Run* runs with the values and keeps them (criterion 9)
1. Resize to 1600 × 1000 with the dialog open, type `{ "id": 123 }` and 2
   ([capture](after-run-inputs-1600x1000-filled.png)), click the dialog's
   *Run*.
2. Select *Cart item* in *Run results*.
3. Click *Run* in the header again.

Expected: PASSED; the *Cart item* step built from the entered values; the
dialog opens again pre-filled with them.
Observed: PASSED, "5 passed"; *Cart item*'s value is `productId : 123`,
`quantity : 2` ([capture](after-run-results-entered-values-1600x1000.png)).
The dialog reopened with `product` and `quantity` pre-filled, both marked
"kept from the last run", *Run* enabled
([capture](after-run-inputs-kept-1600x1000.png)). The object is shown
re-indented (`{` / `"id": 123` / `}` on three lines), as the JSON fields of
the other dialogs show it. Test:
`ktlo23b_the_entered_values_run_and_are_kept_for_the_component` (with
`{"id": 124}` and 2, the run's *Cart item* is `{productId: 124, quantity: 2}`;
the second run uses the kept values; a copy of the component in another file
opens with `product` empty and `quantity` 1).

### Scenario 4: a component whose inputs all have defaults (decision 1)
1. Open `product.afs` (its `id` defaults to 123), click *Run*.

Expected: no dialog; the run starts at once.
Observed: as expected, PASSED, "2 passed"
([capture](after-product-runs-with-no-dialog-1600x1000.png)). Test:
`ktlo23b_a_component_whose_inputs_all_have_defaults_runs_with_no_dialog`;
the KTLO-23a run tests on `product.afs` pass unchanged.

### Scenario 5: scenario runs unchanged
Observed: *Run* on the demo scenario in the same session reported PASSED,
"8 passed", with no dialog and the *Continue remaining steps* box
([capture](after-scenario-run-1600x1000.png)).

### Accessibility
`the_run_inputs_dialog_names_every_control` audits the window with the
dialog open in Diagram, Split and Code at 960 × 540 and 1600 × 1000: no
unnamed control. Each field is named by its title and type ("product ·
Object", "Quantity · quantity · Number"); its title and type line stay
visible text above it.

### How typed text becomes a value
The runner takes JSON values for a component's inputs and does not check
their types, so the dialog does: a text input takes the text as typed; a
number input a finite number (`2`, `1.5`); a yes/no input `true` or `false`;
an object input a JSON object, a list input a JSON array, an input of any
type any JSON (`null`, `"a"`, `[1]`, …). A field holding only spaces counts
as empty. Unit test: `typed_text_becomes_a_value_of_the_inputs_type`. What
cannot be entered is listed in the ticket.

## Windows

Not run on Windows by hand. CI runs the whole suite, including the KTLO-23b
tests, on the Windows runner and uploads the standard captures
(`qa-captures-windows-2022`), which show the scenario tab and no dialog; see
the pull request's checks. Not accepted for Windows on this report alone.

## Not verified

- In the release binary: *Escape* closing the dialog, *Run options → Run
  live requests* through the dialog, a component with invalid code (the
  last-valid prompt, then the dialog), and a second component's own values;
  the tests cover Escape and the second component, not the live and
  last-valid paths through the dialog.
- Inputs of type text, yes/no, list or any in the window: the demo has only
  an object and a number; the unit test covers the conversion of each type.
- Ctrl+Enter to run from the dialog (the dialogs' shared chrome provides it).
- Double scale (960 × 540 at 2×) design review.
- Windows, other than the CI suite and the `qa-captures-windows-2022`
  artifact, which shows no dialog.

## Regression smoke list

Linux, 2026-10-27, release binary of this branch under Xvfb. Item 6 is the
one this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr stayed empty for the whole session. |
| 2 | Pass: the explorer showed the tree; the scenario opened in Code, `add-to-cart.afs` and `product.afs` from the explorer. |
| 3–5, 7–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 6 | Pass: *Run* (simulation) on the scenario reported every step, PASSED, 8 passed; on `add-to-cart.afs` through the dialog, PASSED, 5 passed; on `product.afs`, PASSED, 2 passed. |
| 10 | Pass: at 960 × 540 the dialog and every header control are inside the window (`after-run-inputs-960x540*.png`). |
