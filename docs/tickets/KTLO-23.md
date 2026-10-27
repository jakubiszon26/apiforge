# KTLO-23: A component runs from its tab the way a scenario does

- Size: L, split into KTLO-23a (M · 45 min · `visible UI`) and KTLO-23b
  (S · 20 min · `visible UI`) · Owner review: 65 min in all · Risk: `visible UI`
- State: Approved 2026-10-27 — decisions 1–4 taken 2026-10-26, mock-up (5)
  approved and header width (6) decided 2026-10-27
- Analysis model: Fable · Implementation model: —
- Mock-up branch: `mock/ktlo-23-component-run` (throwaway; captures in
  `docs/qa/2026-10-27/KTLO-23/`, posted on #53)
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

Approved as a whole on 2026-10-27 (decision 5). Captured on the throwaway
branch `mock/ktlo-23-component-run` (commit `fad2184`) under
`docs/qa/2026-10-27/KTLO-23/` and posted on #53:

- **Header**, the demo shop's `components/add-to-cart.afs` active:
  `mock-header-component-960x540.png`, `mock-header-component-1600x1000.png`;
  the header row at 960 × 540 with a long project name:
  `mock-header-long-name-component-620-960x540.png` (today's 620 px),
  `mock-header-long-name-component-650-960x540.png` and
  `mock-header-long-name-scenario-650-960x540.png` (650 px).
- **Run results**, Diagram, 1600 × 1000:
  `mock-run-results-component-1600x1000.png` — the component's five steps,
  the `cart` export selected with its value under *Outputs*, the same node
  selected on the component's board with its inspector open, and no
  error-policy toggle. The panel is drawn 380 px tall instead of the default
  320 px so that the fifth step is in view. Its content is staged, not
  computed: nothing can run a component yet, so the throwaway feeds the
  panel a hand-made run record in the shape of a scenario run (sample
  responses, invented durations).
- **Run inputs** (KTLO-23b): `mock-run-inputs-960x540.png` and
  `mock-run-inputs-1600x1000.png` as the dialog opens (`product` has no
  default and is empty, so *Run* is disabled; `quantity` is pre-filled with
  its default 1), `mock-run-inputs-1600x1000-filled.png` with values entered
  and *Run* enabled.

What the header measures at 960 × 540: with a component tab the toolbar from
*Code* to *Run* is 635 px wide, against 565.8 px on a scenario tab and the
620 px the header reserves — the same in Code, Diagram and Split, and at
1600 × 1000. With the demo name everything fits. With the long project name
of the header test ("Customer integration and contract verification
workspace") the title overlaps *Code* by 14.4 px; with 650 px reserved the
gap is 17.6 px (the test asks for 8) and the title is 192 px wide instead of
224, truncated as today. Hence decision 6. The order is today's: *Publish*
already sits beside *Run options*, so nothing moves.

## Owner decisions, taken 2026-10-26 and 2026-10-27

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
5. **The mock-up is approved as a whole** (2026-10-27, commit `fad2184` on
   `mock/ktlo-23-component-run`): *Run* and *Run options* beside *Publish* on
   a component tab, the *Run results* panel with the component's steps and
   the export's value, the *Run inputs* dialog with *Run* disabled until
   every input has a value.
6. **The header keeps 650 px for the toolbar on every tab** (A, 2026-10-27;
   today 620), forced by a component tab's 635 px toolbar at 960 px. With a
   long project name the title shrinks from 224 to 192 px, clears *Code* by
   17.6 px, and keeps its width when switching tabs. Declined: B, moving
   *Publish* into the *Run options* menu on a component tab, because it
   hides a visible button; C, 650 px only while a component tab is active,
   because the title's width would change between a scenario tab and a
   component tab.

## Acceptance criteria

1. (23a) With a component tab active, the header holds *Run* and *Run
   options* next to *Publish*; at 960 × 540 every header control is inside
   the window (`assert_within_window`), in Code, Diagram and Split.
2. (23a) Clicking *Run* on a component whose inputs all have defaults runs
   it: the *Run results* panel opens, lists each of the component's steps
   with its status, and an `export` step shows the exported value; the
   status is *Passed* when every step passed. The fixture is such a
   component — not the demo's `add-to-cart`, whose `product` has no default,
   so under 23a alone its run fails with "Missing required input: product";
   the demo's `Product` component (`id` = 123) is a candidate.
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
11. (23a) On a component's `export` step the *Run results* panel offers no
    *Check* or *To variable* that can be clicked (absent or disabled): there
    they would edit the component's own graph. This is the agent's technical
    decision, found in the mock-up and not taken by the owner; it is reported
    here so that the owner can overrule it.

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
  `DocumentKind::Scenario`; widen to both kinds. Nothing is reordered:
  *Publish* already sits beside *Run options* (mock-up). The toolbar
  reservation in `header` (`let reserved = 620.0 + …`) goes from 620 to
  650 px on every tab (decision 6); the read-only badge's 80 px stays on
  top of it. The header test
  `long_project_names_do_not_overlap_header_actions_in_writable_or_read_only_windows`
  (`folder/tests.rs:538`) gains a component-tab case, with *Publish* among
  the actions that stay inside the window. `runs.rs::start_prepared_run` takes
  `project.scenarios.first()`; branch on the prepared document's kind.
  `run_report` reads `error_policy` from a scenario — a component has none;
  pass `ErrorPolicy::Stop` and hide the policy toggle (`report::Context`).
  `run_prompt` prints "Scenario: …" from `IdentityBase::Scenario`; add the
  component arm.
- **Report panel (23a), found in the mock-up.** The empty-state hint
  "Results will appear while the scenario runs." (`report.rs:521`) must not
  say scenario when a component runs. *Check* and *To variable* on a
  component's `export` step (`report.rs`, `Action::AssertField` /
  `Action::AssignField`) would edit the component's own graph; they are
  absent or disabled there (criterion 11, the agent's technical decision).
  At the default 320 px panel height the fifth step is cut off and the list
  does not scroll to the selected step: today's behaviour for scenarios too,
  out of scope here, noted in the backlog's *Later*.
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
