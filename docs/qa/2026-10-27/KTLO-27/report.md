# KTLO-27: An empty optional input in the Run inputs dialog means "absent" — QA report

- Branch: `feat/ktlo-27-optional-inputs`; captures from the release binary
  of `cbcf202` (the implementation on top of `main` at `234853c`, KTLO-23b
  included)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Compared against mock-up: the KTLO-23 *Run inputs* dialog
  ([docs/tickets/KTLO-23.md](../../../tickets/KTLO-23.md),
  `../KTLO-23/mock-run-inputs-*.png`) and the KTLO-23b captures
  (`../KTLO-23b/after-run-inputs-*.png`). No new mock-up: the owner's
  decision of 2026-10-27 on #54 puts the optional marker as text beside the
  type.
- Verification on `cbcf202`: `cargo fmt --all -- --check`, `cargo clippy
  --workspace --all-targets --locked -- -D warnings`, `cargo test
  --workspace --all-targets --locked --no-fail-fast` (isolated `XDG_*`
  directories; 1102 passed, 0 failed) and `cargo run -p apiforge-convert-v1
  --locked -- --fixtures --check`: all pass. The five KTLO-27 tests (three
  acceptance tests, the audit and the dialog's unit test) passed ten runs in
  a row. The acceptance commit (`0e9e286`) failed four of them for the
  right reason: no field was named "note · Text · optional", and the
  dialog's *Run* stayed disabled with the optional field empty.

## Captures

`after-*.png` are from the **release binary** of this branch, started with
`--managed` on an empty data directory (which created the demo shop) under
Xvfb with Lavapipe and no window manager. Every state was reached by
**clicking and typing**: a script sent real pointer and key events through
the X server's XTEST extension and captured the window with the X
`GetImage` request; `--screenshot` was not used, because it cannot click.
The window opened at 1600 × 1000 and was resized through the X server to
960 × 540 and back.

The demo has no optional input, so the component of the acceptance tests
was added: `components/notes.afs` was copied into the project folder, then
registered by right-clicking it in the explorer → *Register in project*.

```text
component Notes "Notes" {
  @required(true)
  input item: object
  @optional
  input note: string
  id = param.item.id
  given = if exists param.note {
    echoed = param.note
  } else {
    fallback = "no note"
  }
  size = length(param.note)
}
```

| View | Capture |
| --- | --- |
| The dialog as it opens, 1600 × 1000 | [after](after-run-inputs-optional-1600x1000.png) |
| The dialog as it opens, 960 × 540 | [after](after-run-inputs-optional-960x540.png) |
| `item` filled, `note` empty, *Run* enabled, 960 × 540 | [after](after-run-inputs-optional-960x540-filled.png) |
| `item` filled, `note` empty, *Run* enabled, 1600 × 1000 | [after](after-run-inputs-optional-1600x1000-filled.png) |
| The run with `note` absent, *size* selected | [after](after-run-results-note-absent-1600x1000.png) |
| *Run* again: `item` kept, `note` still empty | [after](after-run-inputs-kept-absent-1600x1000.png) |
| The run with `note` = `hello`, *echoed* selected | [after](after-run-results-note-hello-1600x1000.png) |
| *Run* again: both kept | [after](after-run-inputs-kept-hello-1600x1000.png) |
| *Add to cart* (no optional input): the dialog as before | [after](after-run-inputs-add-to-cart-unchanged-1600x1000.png) |

Compared with the KTLO-23b dialog: one more clause in the hint when the
component has an optional input, and "optional" after the type on an
optional field's type line. Eyebrow, title, field titles, the required
field, the note, *Cancel* and *Run* are as before; *Add to cart*'s dialog
is unchanged ([capture](after-run-inputs-add-to-cart-unchanged-1600x1000.png)
against `../KTLO-23b/after-run-inputs-1600x1000.png`).

## Linux

### Scenario 1: an empty optional field does not block *Run* (criterion 1)
1. Open `notes.afs`, switch to Diagram, click *Run* in the header.
2. Resize to 960 × 540; type `{ "id": 7 }` into `item`; resize to
   1600 × 1000.

Expected: the dialog, "COMPONENT · NOTES"; the hint adds "unless it is
optional: left empty, it is absent"; `item` "Object · no default, enter a
value to run"; `note` "Text · optional · empty means absent"; *Run*
disabled until `item` has a value, then enabled with `note` empty.
Observed: as expected at both sizes; the keyboard focus was in `item` (the
first empty required field). *Run* turned enabled as soon as `item` held
valid JSON, with `note` empty
([960](after-run-inputs-optional-960x540-filled.png),
[1600](after-run-inputs-optional-1600x1000-filled.png)). Test:
`ktlo27_an_empty_optional_input_does_not_block_run` (both sizes; also
*Run* disabled with only `note` typed; `assert_within_window`,
`assert_disjoint`).

### Scenario 2: left empty, the input is absent (criterion 2)
1. Click the dialog's *Run* with `note` empty.

Expected: no value for `note`: `given` takes its `else` path, and `size`,
which reads `note`, fails on its missing value, as in a call that omits the
argument — not as "Missing required input".
Observed: FAILED, "3 passed 1 failed": *id* and *given* passed, *echoed*
skipped, *fallback* passed; *size* Error "length: Operand `left` of Length
expects any, got missing." with Inputs 0
([capture](after-run-results-note-absent-1600x1000.png)). Test:
`ktlo27_an_empty_optional_input_passes_no_value` (also: *size* has no input
recorded; `id` is 7).

### Scenario 3: a typed optional value is passed and kept (criterion 3)
1. Click *Run* in the header again.
2. Type `hello` into `note`, click the dialog's *Run*, select *echoed*.
3. Click *Run* in the header again.

Expected: the first reopening keeps `item` and leaves `note` empty, as it
was run; the run with `hello` takes the `then` path; the next reopening
keeps both.
Observed: `item` "Object · kept from the last run" with `{ "id": 7 }`,
`note` empty, "Text · optional · empty means absent", focused, *Run*
enabled ([capture](after-run-inputs-kept-absent-1600x1000.png)). The run:
PASSED, "4 passed"; *echoed* `value : "hello"`, *fallback* skipped
([capture](after-run-results-note-hello-1600x1000.png)). Reopened: `note`
`hello`, "Text · optional · kept from the last run"
([capture](after-run-inputs-kept-hello-1600x1000.png)). Test:
`ktlo27_a_typed_optional_value_is_passed_and_kept` (also emptied again: the
next run takes `else` and the field stays empty).

### Scenario 4: components without an optional input are unchanged
1. *Cancel*; open `add-to-cart.afs` from Files, click *Run*.

Expected: the KTLO-23b dialog as it was.
Observed: as expected: the one-sentence hint, `product` and `Quantity ·
quantity` as in `../KTLO-23b/after-run-inputs-1600x1000.png`, *Run*
disabled ([capture](after-run-inputs-add-to-cart-unchanged-1600x1000.png)).
The KTLO-23b tests pass unchanged.

### Accessibility (criterion 4)
`the_run_inputs_dialog_names_an_optional_field_as_optional` opens the
dialog on `Notes` and finds the field named "note · Text · optional"; the
audit then finds no unnamed control in Diagram, Split and Code at
960 × 540 and 1600 × 1000. A required field keeps its name ("item ·
Object"; the demo's "product · Object").

### Seen along the way, not part of this change
Registering `notes.afs` while it was open as text left two tabs named
`components/notes.afs` (the text tab and the component tab); closing one
left a single `notes.afs` tab. Registration is untouched by this change;
recorded for the owner.

## Windows

Not run on Windows by hand. CI runs the whole suite, including the KTLO-27
tests, on the Windows runner; its standard captures
(`qa-captures-windows-2022`) show the scenario tab and no dialog. Not
accepted for Windows on this report alone.

## Not verified

- An optional input of type number, yes/no, list or any, or one with a
  contract, in the window: only an optional text input was run; the marker
  and the rule do not depend on the type (unit test
  `only_a_declared_optional_input_without_a_default_may_stay_empty`).
- `@required(false)`, the long form of `@optional`, in the window: it
  lowers to the same declaration (ADR 0005); the unit test covers the
  declaration, not the source.
- A call from a scenario that omits the optional argument, side by side
  with the dialog's run: the component would have to be published first.
  The acceptance test checks that no value reaches the input, which is what
  a call without the argument binds.
- Escape, *Run options → Run live requests* through the dialog, Ctrl+Enter.
- Double scale (960 × 540 at 2×) design review.
- Windows, other than the CI suite.

## Regression smoke list

Linux, 2026-10-27, release binary of `cbcf202` under Xvfb. Item 6 is the
one this change touches.

| # | Result |
| --- | --- |
| 1 | Pass: `--managed` with an empty data directory created and opened the demo shop; stderr stayed empty for the whole session. |
| 2 | Pass: the explorer showed the tree; the scenario opened in Code, `notes.afs` and `add-to-cart.afs` from the explorer. |
| 3–5, 7–9 | Not run by hand for this change; the workspace suite that covers them passes, and so does `apiforge-convert-v1 --fixtures --check`. |
| 6 | Pass: *Run* (simulation) on `notes.afs` through the dialog, with `note` absent (FAILED as the component is written, 3 passed 1 failed) and with `note` = `hello` (PASSED, 4 passed); *Add to cart* opens its dialog as before. |
| 10 | Pass: at 960 × 540 the dialog and every header control are inside the window (`after-run-inputs-optional-960x540*.png`). |
