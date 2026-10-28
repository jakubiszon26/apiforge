# KTLO-32: An input without a default and without `required` is required in the runner too

- Size: S · Owner review: 10 min · Risk: `default` (the runner's reading of
  such an input changes — the owner's decision of 2026-10-28 on #62,
  recorded on #112) · `visible UI` (the required marker the board and the
  inspector already show for `@required(true)` inputs now also appears for
  these; a consequence of the decision, no new control)
- State: Analysis approved — the owner's decision of 2026-10-28 (quiz on
  #62, recorded on #112): an input with no default and no `required` flag
  is required everywhere; this file is the analysis it asked for.
- Analysis model: Fable · Implementation model: —
- Found in the review of #109; charged to the sprint's UX line.

## Goal

A component input declared with neither `@optional`/`@required(false)` nor a
default is required wherever the component is used: the Run inputs dialog
and the MCP `run` tool already ask for it; now the runner refuses to run
without it, and a call to the component marks it as required on the board.

## What the owner will see

- Changed: a component run alone (the window's *Run*, MCP `run`) that lacks
  such an input fails the first step reading it with *Missing required
  input: <name>.*, as it does for a `@required(true)` input, instead of
  running on with the input absent. A scenario calling the component
  (`handle@n(…)`) without that argument fails the call step with the same
  message. On the board the call card's row for the input shows the missing
  marker instead of the optional one; the inspector labels it `<name> *` and
  says *Set this required argument.* while it is empty. The dialog and the
  MCP tool behave as today, now by the shared rule.
- Unchanged: the language, printing and the file format — `required` stays
  `Option<bool>` in the model and the printer writes `@required(true)` only
  where it was written, so every document round-trips byte-identical;
  inputs with a default; `@optional` inputs (KTLO-27); the built-in ports
  (Request `body`, headers and query, Export `value`, Condition's data),
  which keep their own reading: this rule is for declared component inputs.
- Removed or hidden, migrated: nothing. Default behaviour that changes: the
  runner's (the owner's decision, #62/#112).

## Mock-up

Not applicable: the marker, the label and the message exist today for
`@required(true)` inputs and are shown unchanged for one more case.

## Acceptance criteria

1. `Port::is_required_input()` in `crates/apiforge-core/src/graph.rs`: true
   unless `required == Some(false)`; `is_required()` is unchanged for the
   built-in ports. `input_ports` for `NodeKind::Component` sets each cloned
   input's `required` to `Some(is_required_input())`, so the engine's step
   check, the board, the inspector and *Extract component* follow. Unit test
   `an_undeclared_component_input_is_required` beside
   `ports_follow_node_kind_and_configuration`.
2. Run alone: a component with `input note: string` (no attribute, no
   default) run without `note` reports the step reading it as *Missing
   required input: note.*; with `@optional` it runs as KTLO-27 says; with a
   default it uses the default. Test
   `ktlo32_an_undeclared_input_is_required_when_run_alone` in
   `crates/apiforge-runner/tests/component_target.rs` (`component_run`,
   `engine.rs:109`, uses the new predicate).
3. Call in a scenario: `handle@1(…)` without the argument fails the call step
   with *Missing required input: note.*; with it the component runs. Test
   `ktlo32_an_undeclared_input_is_required_on_a_call`, same file.
4. Board and inspector: with the call card selected, the row for `note` is
   the missing marker and the inspector shows `note *` with *Set this
   required argument.*; binding a value clears both. Interaction test
   `ktlo32_an_undeclared_input_is_marked_required_on_a_call` in
   `crates/apiforge-desktop/src/folder/editor_tests.rs`, both window sizes.
5. One rule: `dialogs/run_inputs.rs` (`optional:`) and
   `crates/apiforge-mcp/src/tools.rs` (the missing-input check and the
   `optional` field) use `is_required_input()`; their existing tests
   (`only_a_declared_optional_input_without_a_default_may_stay_empty`, the
   MCP-03 criterion 5 tests, KTLO-27's) pass unchanged.
6. Shipped demos and fixtures do not change behaviour: `demos.rs` declares
   every input `.required(true)`; `fixtures/workspace.json` has `required:
   true` on all six inputs; `fixtures/scripts/lookup.afs` (`input productId:
   string`) is parsed and printed by `apiforge-script` tests only and still
   round-trips. Regression smoke items 6 and 9 on the release binary.
7. ADR 0005 gains a dated amendment under the input attributes: an input with
   neither attribute and no default is required (#62, 2026-10-28).
8. The verification commands of the development guide pass.

## Not in scope

Changing the language or the printer (writing `@required(true)` for such an
input); an input with a default left absent; a `null` value for an input.

## Regression potential and what covers it

Any test component built with `Port::new` and no `.required(...)` that is
run without the input now fails: the workspace suite finds them (a search
found none outside `demos.rs`, where every input is required). The built-in
ports are untouched by construction (criterion 1).

## Technical appendix

- Callers of `is_required()` today: `engine.rs:109` (component run alone,
  changes), `engine.rs:799` (every step's ports; unchanged, but receives the
  normalised component ports), `canvas/layout.rs:303,619` and
  `inspector/binding.rs:155,174` (follow through `input_ports`),
  `edit/component.rs:233` (*Extract component*, follows).
- Files: `crates/apiforge-core/src/graph.rs`,
  `crates/apiforge-runner/src/engine.rs`,
  `crates/apiforge-desktop/src/dialogs/run_inputs.rs`,
  `crates/apiforge-mcp/src/tools.rs`, the tests named above,
  `docs/decisions/0005-scenario-language.md`.
