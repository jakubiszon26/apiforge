# KTLO-27: An empty optional input in the Run inputs dialog means "absent"

- Size: S · Owner review: 10 min · Risk: `visible UI`
- State: Awaiting acceptance (issue #84; branch
  `feat/ktlo-27-optional-inputs`)
- Actual: about 530 changed lines without captures — over S (≤ 300) with
  the documents, within S by code and tests alone: code 54 (the dialog),
  tests 207 (desktop acceptance 159, audit 25, the dialog's unit test 23),
  QA report 181, this ticket 89, backlog 1; one round
- Analysis model: Fable · Implementation model: Opus 5.5
- After KTLO-23b (#54, PR #80); the dialog is KTLO-23b's

## Goal

In the *Run inputs* dialog an optional input (one the component declares as
optional, with no default) does not block *Run*; left empty it passes no
value, and the run behaves as when a caller omits it. A required input still
blocks *Run* until it has a value.

## Decision

Owner decision of 2026-10-27 on #54 (KTLO-23b, PR #80): an empty optional
field means "absent", in a separate S ticket. The KTLO-23 mock-up covers the
dialog; the optional marker is text beside the type, so no new mock-up.

## What the owner will see

- **Changed:** an optional field's type line reads "Text · optional · empty
  means absent" while it is empty, and "Text · optional · no default" or
  "… · optional · kept from the last run" once it holds a value. Its
  accessible name ends with the marker ("note · Text · optional"). The
  dialog's *Run* is enabled once every required field has a valid value;
  an optional field may be empty, and a value it holds must still be of its
  type. The hint at the top adds "unless it is optional: left empty, it is
  absent" when the component has such an input.
- **Unchanged:** components without an optional input (the whole demo shop)
  show the dialog exactly as before; required fields; the dialog appears
  only when some input has no default (decision 1 of KTLO-23); *Cancel*;
  the values kept for the session.
- **Removed or hidden, migrated, default changed:** nothing.

## Acceptance criteria

1. With a component whose inputs are one required and one optional without
   defaults, the dialog enables *Run* once the required field has a value
   (both window sizes).
2. Running with the optional field empty passes no value: a step reading it
   fails as it does when a caller omits the input; a step with a default
   path runs that path.
3. A typed optional value is passed and kept for the session as other
   values are.
4. The accessibility audit passes; the optional marker is part of the
   field's name.
5. The verification commands pass.

Tests: 1 `ktlo27_an_empty_optional_input_does_not_block_run` (both sizes),
2 `ktlo27_an_empty_optional_input_passes_no_value`, 3
`ktlo27_a_typed_optional_value_is_passed_and_kept`, all in
`folder/component_run_tests.rs`; 4
`the_run_inputs_dialog_names_an_optional_field_as_optional` in
`folder/accessibility_tests.rs`; the rule in the dialog's unit test
`only_a_declared_optional_input_without_a_default_may_stay_empty`. The
design review view is `review_run_inputs_optional`.

## What the implementation found

- **What "optional" is.** The language declares it with `@optional` or
  `@required(false)` (ADR 0005). An input with neither attribute is not
  marked optional and is still asked for, as in KTLO-23b, although the
  runner treats it as not required (`Port::is_required` is false for it):
  a caller may omit it without a validation error. No demo input is
  undeclared. For the owner, if it matters: such an input could be treated
  as optional too.
- **An optional input with a default** is not marked: it is pre-filled with
  its default as any other, and emptied it blocks *Run*. Left absent, the
  runner would use the default anyway.
- **No value, not `null` or `""`.** The empty field is left out of the
  values the dialog passes and keeps; the runner then binds nothing to the
  input, as it does for a call without the argument. The acceptance test
  tells these apart: `exists param.note` takes its `else` path (it would
  take `then` for `null` or `""`), and the step `length(param.note)` fails
  on its missing operand with no input recorded, not as "Missing required
  input".
- **Kept as absent.** A run with the optional field emptied keeps it empty
  for the next run of the session, as other values are kept.
- The keyboard focus goes to the first empty *required* field, then to the
  first empty one.

## Not in scope

Treating an undeclared input as optional; leaving an input with a default
absent; a `null` or empty-text value for an input.
