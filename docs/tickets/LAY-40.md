# LAY-40: A scenario opens in Diagram in a project never opened here

- Size: S · Owner review: 5 min · Risk: `default` (which view a project
  first opens in)
- State: decided by the owner on 2026-11-18 (Q8 = A, #296): a scenario in a
  project never opened on this computer opens in Diagram; a remembered
  choice still wins. Accepted on a capture before merge (#296, Plan A) ·
  LAY sprint, part of its close
- Analysis model: Fable · Implementation model: Opus

## Goal

A tester who opens a project for the first time is asked to read the flow,
and the window shows the file's text: Diagram is one click away, and the
first thing read is a list of warnings in *Problems*. The first view of a
scenario is the board.

## What the window does today

Looked at on `main` 2f6922c with the room project copied fresh and no
settings (1280 × 800): the project opens with its first scenario in a tab,
in **Code** (`folder/workbench.rs`: "a project without one starts in
Code"). The view a person leaves a project in — Code, Diagram or Split —
is remembered per project on this computer and comes back on the next
open; that works and stays.

## What the owner will see

- Default behaviour that changes (decided, Q8 = A): a project with **no
  remembered view on this computer** whose first tab is a **scenario**
  opens in **Diagram**. The board is the one LAY-33 gives on a first open:
  the Tidy up shape, fitted.
- Unchanged:
  - A project opened here before comes back in the view it was left in,
    Code included.
  - A project with no scenario (only components, or nothing yet), and a
    project just created in an empty folder, open in Code as today; the
    component view is not part of this decision.
  - `--mode` on the command line shows what it asks for.
  - The switch Code / Diagram / Split, Ctrl+E, what each view shows, what
    is saved. Opening a project still writes nothing to its files.
- Removed or hidden, migrated: nothing. No setting is added.

## Mock-up

Not applicable as a drawing: the view is the Diagram that exists. The pull
request carries one capture for acceptance before merge — the room project
(`fixtures/scenarios/lay/rooms/`, added by LAY-39 and LAY-15; until one of
them is merged, a fresh copy of the demo shop project) at its first open,
1280 × 800.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay40.rs`
unless named otherwise; each opens a temporary copy of the demo shop
project with no stored view (its own data directory, as the harness gives).

1. First open, first tab a scenario: the mode is Diagram, the Diagram
   switch is the selected one, and the board of that scenario is drawn (it
   has cards).
2. A stored view wins: the project is opened, switched to Code, closed and
   opened again → Code. The same with Split → Split. And a project left in
   Diagram → Diagram.
3. A project with no scenario (the copy with its scenarios removed from the
   manifest and the folder) opens in Code; a project created by *New
   project* in an empty folder opens in Code (`folder/tests.rs`, the
   existing test, unchanged).
4. A mode asked for at start-up (`request_mode`, as `--mode code`) is the
   one shown on a first open, and — as today — is not what the project
   remembers afterwards.
5. A first open writes no file under the project folder that `main` does
   not write (the folder's listing before and after is compared with the
   same open on a stored Code view).
6. `workbench.rs`: `Workbench::restore(None, …)` documents and tests the
   rule it implements; the unit test "a new project starts in Code" is
   replaced by the rule's two halves (with a scenario, without one).

## Regression potential

About 260 existing tests open a folder without asking for a mode
(`testing::folder_harness`, `folder_harness_scaled`, `open` in
`folder/tests.rs`) and many read the code editor straight away. They are
not this ticket's subject and must keep testing what they test: the shared
helpers keep giving them Code (the implementer chooses how — a stored view,
or the start-up request — and says which in the pull request), and only
tests that assert the first view itself change: `a_first_open_shows_the_
first_scenario_in_code` (`folder/tests.rs`) and the first assertion of
`the_mode_switches_change_the_view` (`folder/interaction_tests.rs`). A test
that fails for another reason is escalated, not edited.

## Not in scope

- What the first Diagram shows at Fit (the Overview line: LAY-41) and
  whether Fit stops at a readable zoom (not decided; #296 Q3 = A chose the
  line).
- Split as the first view; a per-document view; a setting for the default.
- The component view's first open.

## Technical appendix

- `folder/mod.rs` `install()`: `first_open = state.is_none()`; the first
  document is chosen there (scenarios first, by path). The rule belongs
  where both facts are known: no stored state **and** that document is a
  scenario. `Workbench::restore(None, …)` is also used for the empty shell
  before a project is loaded (`shell()`), which must stay Code — an empty
  window has no board.
- `mode_request` is applied in `install()` before the first-open block and
  must keep winning.
- The stored view is `WorkspaceState` in the application's data directory,
  keyed by the manifest's id (`opening.rs`), not in the project folder.
