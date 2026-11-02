# KTLO-32: An input without a default and without `required` is required in the runner too — QA report

- Branch: `fix/ktlo-32-required-inputs`. Captures and command output are
  from the release binaries of `6c86932` (`apiforge-desktop`, `apiforge`).
- Built with: `cargo build --release --locked -p apiforge-desktop -p apiforge-cli`
  (Rust 1.99.0).
- Compared against mock-up: none; the ticket has none
  ([`docs/tickets/KTLO-32.md`](../../../tickets/KTLO-32.md), *Mock-up*): the
  marker, the `name *` label and *Set this required argument.* are the ones
  `@required(true)` inputs already show.
- Analysis model: Fable. Implementation model: Claude Opus 5.5.
- Acceptance tests first: commit `4aa3de1`. Before the implementation the
  four KTLO-32 tests failed on their assertions, for the reason the ticket
  gives: the call's `note` port was optional (`required: None`), the board
  painted `note` and *Optional* and the inspector `note` without a message,
  and both runs passed with `note` absent. The commit carries a stub
  `Port::is_required_input()` with the old reading so that the tests
  compile; the implementation commit `6c86932` replaced it and changed no
  assertion.

## Criteria and their evidence

| # | Criterion | Test | Release binary |
| --- | --- | --- | --- |
| 1 | `is_required_input()`; a call's ports carry it; built-in ports unchanged | `graph::tests::an_undeclared_component_input_is_required` (apiforge-core) | Scenario 1 |
| 2 | Run alone without `note`: *Missing required input: note.*; `@optional` runs on; a default is used | `ktlo32_an_undeclared_input_is_required_when_run_alone` (`apiforge-runner/tests/component_target.rs`) | Scenario 3 (MCP `run`) |
| 3 | `notes@1()` fails the call step with the message; with the argument it runs | `ktlo32_an_undeclared_input_is_required_on_a_call` (same file) | Scenario 2 |
| 4 | Board row is the missing marker; inspector `note *` with *Set this required argument.*; binding a value clears marker and message; both sizes | `ktlo32_an_undeclared_input_is_marked_required_on_a_call` (`apiforge-desktop/src/folder/editor_tests.rs`) | Scenario 1 |
| 5 | The dialog and the MCP tool use the shared rule; their tests pass unchanged | `only_a_declared_optional_input_without_a_default_may_stay_empty`, the MCP-03 and KTLO-27 tests, unchanged and passing | Scenario 3 |
| 6 | Shipped demos and fixtures do not change behaviour | full suite; `--fixtures --check` | Regression smoke 6 and 9 |
| 7 | ADR 0005 amendment | `docs/decisions/0005-scenario-language.md`, input attributes | — |
| 8 | Verification passes | see *Verification* | — |

One reading the ticket left open is recorded in its *Implementation notes*:
an undeclared input **with** a default is not marked required, so its row on
a call stays *Optional* as before; the runner gives it the default either way.
The same notes record a second change: *Edit component* now starts an
undeclared public argument's *Required* box ticked, so saving no longer
turns it optional, and one with a default unticked, so saving no longer
makes it required (test
`an_undeclared_public_argument_stays_required_when_the_component_is_edited`;
review round 1, after the captures: it changes no flow captured here).

## How the release binaries were driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
scratch configuration, data and runtime directories. `fixtures/workspace.json`
was migrated by the release window's managed start (`--managed`), which
wrote the four folder projects. The release `apiforge mcp --folder …/shop`
was then driven over stdio by a scratch script (not committed) to add
`components/notes.afs`, publish its revision 1, and add
`scenarios/write-a-note.afs`:

```afs
component notes "Notes" {
  input note: string
  export text: string = param.note
}
```

```afs
scenario "Write a note" {
  write_a_note = notes@1()
}
```

The window was opened with `--folder …/shop` at 1600 × 1000. Clicks went
through the X test extension (libXtst from the Ubuntu archive, unpacked into
a scratch directory, not installed); the screen was grabbed from the display,
and the 960 × 540 window is the same process resized through Xlib.

## Linux

### Scenario 1 (criteria 1, 4): the call card and the inspector

1. Open `write-a-note.afs` from the explorer; switch to *Diagram*.
2. Click the `write_a_note` card.

Expected: the card's row reads `note *` over *Value missing*; the inspector's
*Arguments* lists `note *`, *Not set*, and *Set this required argument.*

Observed: as expected at 1600 × 1000
([capture](board-inspector-1600x1000.png), before the run). At 960 × 540
([capture](board-inspector-960x540.png), taken after Scenario 2, so the card's
footer reads *Error · 0 ms*; the inspector is scrolled to *Arguments*, the
run results collapsed) the card and the argument are both fully visible and
nothing overlaps.

### Scenario 2 (criterion 3): a run that lacks the input

1. With the card selected, click *Run*.

Expected: a simulation that fails at its only step, `write_a_note`, with
*Missing required input: note.*, and no step of the component runs.

Observed: as expected: *FAILED*, *0 passed · 1 failed*, the step *Error* with
*Missing required input: note.*, *Outputs 0*, *Inputs 0*
([capture](run-missing-input-1600x1000.png) at 1600 × 1000,
[capture](run-missing-input-960x540.png) at 960 × 540). The same scenario
from the command line:

```text
$ apiforge run --folder …/shop --scenario scenarios/write-a-note.afs
    error  write_a_note — Missing required input: note.
Failed
(exit code 1)
```

The MCP `run` tool on the scenario returned `status: "failed"`, one step
`write_a_note`, `status: "error"`, `message: "Missing required input:
note."`, `may_have_reached_api: false`.

### Scenario 3 (criteria 2, 5): the component run alone through MCP

1. `run` with `path: "components/notes.afs"` and no `inputs`.

Expected: refused before running, as before KTLO-32 (the tool already
asked for such an input), now by the shared rule.

Observed: `invalid_inputs`, `missing: ["note"]`, and for `note`
`has_default: false`, `optional: false`; nothing ran (`wrote: false`).

## Windows

Not run on Windows: no Windows machine is available to the agent. CI on
`windows-2022` runs the same test suite, including the four KTLO-32 tests;
its `qa-captures-windows-2022` artifact shows the demo shop only, not this
scenario. The ticket is not accepted for Windows on this report.

## Not verified

- The run alone from the window's component tab with an empty *note* in the
  Run inputs dialog: the dialog's behaviour is unchanged and covered by its
  unit test; the runner's half by criterion 2's test, not by the release
  binary.
- Binding a value on the release binary (criterion 4's second half): covered
  by the interaction test at both sizes, not by a capture.
- *Edit component* on the release binary: covered by the unit test named
  above only.
- Windows, as above. Screen reader session: not run.

## Regression smoke list

Linux, 2026-11-02, release binaries of `6c86932`:

| # | Flow | Result |
| --- | --- | --- |
| 1 | Start with no arguments | Not run as such; `--managed` (the same start) migrated the legacy workspace and opened a project, nothing on stderr |
| 2 | Open a folder project | Pass: `--folder …/shop` showed the tree; the shop scenario opened in Code |
| 3 | Add a block from the library panel | Not run |
| 4 | Wire two blocks | Not run |
| 5 | Edit source, see the board | Not run |
| 6 | Run a simulation | Pass: `apiforge run --folder …/shop --scenario scenarios/promocja-i-cena-w-koszyku.afs` passed every step including cleanup; no `--live`, no request sent |
| 7 | Save (Ctrl+S) and reopen | Not run |
| 8 | Close a dirty tab, kill with unsaved work | Not run |
| 9 | Migrate a legacy workspace | Pass: four projects, each passes `apiforge check --folder`; `workspace.json` byte-identical (SHA-256 before and after) |
| 10 | Window at 960 × 540 | Pass for Diagram with the inspector and the run results open (Scenarios 1 and 2); Code and Split not captured |

## Verification

On `6c86932`, Linux: `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets --locked -- -D warnings`, `cargo test --workspace
--all-targets --locked` (no failures; the desktop suite 539 passed, 40
ignored) and `cargo run -p apiforge-convert-v1 --locked -- --fixtures
--check` passed.
