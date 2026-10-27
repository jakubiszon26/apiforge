# KTLO-23b: A component runs from its tab — the Run inputs dialog

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: Awaiting acceptance (issue #54; branch `feat/ktlo-23b-run-inputs`)
  — approved 2026-10-27 (mock-up)
- Actual: about 890 changed lines without captures — over S (≤ 300) by
  the code alone, and over M (≤ 800) with the QA report: code 335 (the
  dialog 243, wiring 92), tests 342 (desktop acceptance 257, audit 19, the
  dialog's unit tests 66), QA report 170, this ticket 41; one round
- Analysis and acceptance criteria: `docs/tickets/KTLO-23.md`
- Analysis model: Fable · Implementation model: Opus 5.5
- After KTLO-23a

## Delivery

A *Run inputs* dialog before a component run: one field per input with its
type, pre-filled with the input's default, values kept for the session per
component. With decision 1 = A it appears only when an input has no default;
with B before every component run. As mocked and approved, the dialog
pre-fills the defaults (`quantity` = 1 in the mock-up) and keeps *Run*
disabled until every input has a value.

Criteria covered: KTLO-23 criterion 9, as an interaction test through the
named dialog widgets, plus the dialog's view in the mock-up.

## What the implementation found

- **How a typed text becomes a value.** The runner takes a JSON value per
  input and does not check its type, so the dialog does: a text input
  takes the text as typed; a number input a finite number; a yes/no input
  `true` or `false` (typed, no toggle); an object input a JSON object, a
  list input a JSON array, an input of any type any JSON. The contract
  named beside an object input is shown, not checked: a value of another
  shape runs.
- **What cannot be entered.** An empty text, or a text of spaces only, for a
  text input: an empty field means "no value", and *Run* waits for every
  input to have one. `null` for an object or list input (an input of any
  type takes it).
- **An optional input without a default must be given a value too.**
  Decision 1 says *Run* waits until every input has a value, so a run with
  an optional input left absent — the path a caller takes when it omits the
  argument — cannot be tried from the tab. No demo component has such an
  input. For the owner, if it matters: an empty optional field could mean
  "absent".
- **The KTLO-23a test of criterion 7** (a missing input fails its step) is
  removed with this ticket, as the criterion foresaw ("until KTLO-23b
  lands"): the dialog keeps such a run from starting. The runner's test of
  the message stays.
- *Cancel* (and Escape) keeps nothing that was typed; only the dialog's
  *Run* stores the values, per component file, until the project is closed.
  A kept value is marked "kept from the last run" instead of "pre-filled
  from its default". The dialog focuses the first empty field.
- The dialog follows the last-valid-version prompt, so it asks for the
  inputs of the version that will run.
- The acceptance tests are in `folder/component_run_tests.rs` beside
  KTLO-23a's; the audit is in `folder/accessibility_tests.rs`.

## Not in scope

Remembering values across sessions or in the project; typed editors beyond
text, number, boolean and literal JSON.
