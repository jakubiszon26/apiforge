# KTLO-23: A component runs from its tab the way a scenario does

- Size: L, split into KTLO-23a (M · 45 min · `visible UI`) and KTLO-23b
  (S · 20 min · `visible UI`) · Owner review: 65 min in all · Risk: `visible UI`
- State: Analysis — decisions 1–3 taken 2026-10-26; mock-up pending
- Analysis model: Fable · Implementation model: —
- Mock-up branch: none yet; made next (see Mock-up)
- Owner's wish of 2026-10-26: when a component is written straight in code it
  cannot be tried out; it should run like a scenario does.

## Goal

With a component's tab active, *Run* simulates that component on its own —
its current text, saved or not — and the *Run results* panel shows every
step and every exported value, exactly as it does for a scenario. A
component no longer has to be called from a scenario to be tested.

## What the owner will see

- **New:** with a component tab active, the header shows *Run* and *Run
  options* beside *Publish*, as it shows them for a scenario today (today a
  component tab has only *Publish* and a *Runs* menu). The run uses the
  component's working text, saved or not, and the published revisions of the
  components it calls; a component with code errors offers the last valid
  version and asks first, like a scenario. The *Run results* panel lists the
  component's steps with the component's own board selectable from them; an
  `export` appears as a step whose value is the output. With decision 1 = A
  (KTLO-23b): when the component has an input without a default, a small
  *Run inputs* dialog asks for the values before the run and remembers them
  for the session.
- **Changed:** the hover text of *Run* reads "Simulate the selected scenario
  or component…". The command line is unchanged (see Not in scope).
- **Removed or hidden:** nothing.
- **Migrated or converted:** nothing. No file changes: the run is in memory,
  and the live-run journal records the component's name where it records a
  scenario's.
- **Default behaviour that changes:** none. Scenario runs are untouched.

## Mock-up

Not produced yet; made on a throwaway branch now that decisions 1–3 are
taken, and posted on the issue before any implementation. Views to mock up:
the header with a component tab active at 960 × 540 and 1600 × 1000 (*Run*,
*Run options*, *Publish*, *Save*, *Save all* must all fit at 960 px — the
header reserves 620 px for the toolbar today, and a component tab shows one
button more than a scenario tab; if they do not fit, the mock-up proposes a
rearrangement and that becomes a further decision); the *Run results* panel
after a component run with an `export` step selected, at 1600 × 1000; the
*Run inputs* dialog at both sizes.

## Owner decisions, taken 2026-10-26

1. **A component's inputs are asked for only when they are needed.** A *Run
   inputs* dialog appears before the run when some input has no default,
   with one field per input pre-filled from the default, and the values are
   kept for the session; a component whose inputs all have defaults runs
   with no dialog. Reason: no extra click in the common case, and nothing
   forces a default to be written into a component's source — which would
   make that input optional for every caller — merely to test it.
2. **The working text runs**, saved or not, as *Run* does for a scenario,
   with the last-valid prompt when the code has errors. Reason: the point of
   the ticket is to try out what is being typed, before publishing.
3. **Live requests are allowed from a component**, through *Run options →
   Run live requests*, with the same consent and the same journal as a
   scenario, recorded under the component's name. Reason: a component is
   where the requests are written, so it is where they must be tried.
4. **The command line is a later ticket**, outside the KTLO sprint: `Run` in
   the window is what the wish asked for, and the command line runs saved
   files only. Recorded under Not in scope.
5. **The mock-up** is still to be approved.

## Acceptance criteria

1. (23a) With a component tab active, the header holds *Run* and *Run
   options* next to *Publish*; at 960 × 540 every header control is inside
   the window (`assert_within_window`), in Code, Diagram and Split.
2. (23a) Clicking *Run* on a component whose inputs all have defaults runs
   it: the *Run results* panel opens, lists each of the component's steps
   with its status, and an `export` step shows the exported value; the
   status is *Passed* when every step passed.
3. (23a) The run uses the unsaved text: editing a value in the code, then
   *Run*, reports the new value; the file on disk is unchanged.
4. (23a) A component that calls another component runs that component's
   pinned published revision; its inner steps are listed indented, as in a
   scenario run.
5. (23a) Invalid code offers *Run the last valid version?* and runs it only
   after *Run this version*, as for a scenario; editing the code after a run
   marks the report stale.
6. (23a) Selecting a step in the panel selects that node on the component's
   board and opens its inspector.
7. (23a) Until KTLO-23b lands, an input without a default
   and without a value fails its step with "Missing required input: name".
8. (23a) *Run options → Run live requests* on a component
   runs it live, journalled with the component's name; an interrupted live
   run is listed at the next start.
9. (23b) A component with an input without a default
   shows the *Run inputs* dialog naming each input and its type; *Run* in
   the dialog runs with the entered values; *Cancel* runs nothing; the next
   run of the same component in the session pre-fills the values.
10. (23a) Scenario runs behave exactly as before: the existing run tests in
    `folder/tests.rs` and `folder/acceptance_tests.rs` pass unchanged.

## Not in scope

`apiforge run --folder` for a component: the command line is "saved files
only" and its argument is `--scenario`; a `--component` flag is a later
ticket outside this sprint (owner decision 4). Running a single step or up to a step of a component (the folder
window has neither for scenarios). Remembering run inputs across sessions or
writing them into the project. Any change to the `.afs` grammar.

## Technical appendix

- **Nothing runs a component standalone today.** `apiforge_runner::engine`
  runs `request.scenario_id` only; a component executes solely as a nested
  `NodeKind::Component` step (`Engine::component`, `engine.rs` ~828), whose
  `Context.parameters` are the call's bound values plus `Port::default()`
  (~715). `PreparedRun::project()` (`apiforge-project/src/execution.rs`)
  returns `Error::InvalidModel` for an `IdentityBase::Component`, and
  `ProjectSession::prepare_run` refuses any non-scenario document
  (`Error::ModelLocked`). The CLI's `folder.rs::execute_run` filters on
  `DocumentKind::Scenario`. The legacy window (`app.rs::start_run`) runs
  `editor.scenario()` only. `rg` finds no "run component" in `docs/`.
- **Runner (23a).** Preferred: a second entry point beside `run_scenario`,
  `RunRequest::target: RunTarget::{Scenario(id), Component{id, revision,
  inputs: IndexMap<String, Value>}}` (or a sibling field keeping
  `scenario_id`), and `Engine::run` building its `Context.parameters` from
  `inputs` + defaults and calling `execute_graph(&component.graph, …)` with
  no cleanup phase. `RunResult.scenario_id` carries the component id and
  `snapshot.graph` the component graph, so `report.rs::flatten_steps` and
  `Action::SelectNode(root_id)` land on the component's own nodes. The
  alternative — a synthetic one-node wrapper scenario — needs no engine
  change but puts every step at depth 1 under a node absent from the board,
  so selection would need unwrapping; rejected. `validate()`
  (`request.rs:153`) and `validate_previous` learn the new target.
  `JournalRun::new` (`engine.rs:405`) takes the component's name.
- **Project (23a).** `prepare_run` accepts `DocumentKind::Component`;
  `PreparedRun::project()` pushes the snapshot's `IdentityBase::Component`
  onto `context.components`. The snapshot's `closed_public_context`
  (`apiforge-storage/src/project/cache.rs:502`) already carries the called
  revisions, the environment and the contracts, so pinned revisions and
  secrets flow as for a scenario. The working model carries its *next*
  revision number (`prepare.rs`, "the model carries its next revision"):
  it must not collide with a published one — `publishable()` guarantees it is
  unused. `run_is_stale` and `validate_run` need no change.
- **Desktop (23a).** `folder/chrome.rs::header`: `run_document` filters on
  `DocumentKind::Scenario`; widen to both kinds and reorder so *Publish*
  stays beside *Run* (check the 620 px reservation and the header test at
  `folder/tests.rs:556`). `runs.rs::start_prepared_run` takes
  `project.scenarios.first()`; branch on the prepared document's kind.
  `run_report` reads `error_policy` from a scenario — a component has none;
  pass `ErrorPolicy::Stop` and hide the policy toggle (`report::Context`).
  `run_prompt` prints "Scenario: …" from `IdentityBase::Scenario`; add the
  component arm.
- **Dialog (23b).** New `dialogs/run_inputs.rs` on the pattern of
  `dialogs/environment.rs` (`centered_modal`, `Outcome`); typed fields per
  `Port.value_type`, literal JSON for object/array; values held in
  `Runs` as `HashMap<component_id, IndexMap<String, Value>>`. Hooked in
  `request_run` before `Prompt::Run`.
- **Tests.** Acceptance in `folder/acceptance_tests.rs` (`choose(&mut
  harness, "Run")`, `finish_folder_run` pattern from `folder/tests.rs:1811`,
  a fixture component with an `export`); runner unit tests beside
  `engine.rs:1368`; review view in `testing.rs` next to
  `review_folder_modes_*`.
- **Size.** 23a: engine ~120, project ~40, desktop ~60, tests ~250, QA ~120,
  docs ~20 ≈ 600 (M). 23b: dialog ~150, wiring ~30, tests ~100, QA ~60 ≈ 340
  (S, at the edge). KTLO-17 ran 2.4× its estimate on tests alone; hence the
  split rather than one ticket.
