# Testing and acceptance

Stage II had 961 passing tests and shipped without a block palette, with a
migration that refuses the owner's real workspace and with three defects that
the first cross-package suite found immediately. A test written in the same
session as the code checks the author's assumptions; it does not check the
owner's criteria. Three layers correct that.

## 1. Acceptance tests first

Each acceptance criterion of an approved ticket becomes a test **before** the
implementation, in its own commit (`test(<crate>): KTLO-05b acceptance for the
block palette`). The commit shows the tests failing for the right reason; the
implementation commits then make them pass without editing the assertions.

- Where they live: `crates/<crate>/tests/acceptance.rs` for a library crate,
  `folder/acceptance_tests.rs` for the desktop. Interface criteria are
  interaction tests through named widgets ([GUI testing](../gui-testing.md)).
- A criterion that cannot be turned into a test is recorded in the ticket as a
  manual QA step with the reason; "hard to test" is not a reason.
- A test that must wait for a decision is `#[ignore = "<ticket>: <reason>"]`.
  An ignore with no ticket is a defect, with two named exceptions that are not
  waiting for anything: a design-review view
  (`#[ignore = "design review capture; run with tools/gui/review.sh"]`) and a
  helper a test spawns as a child process
  (`#[ignore = "launched by …"]`). Any other reason names a ticket.

The whole suite still passes with the commands of the
[development guide](../development.md#verification).

## 2. The functional QA report

Since the owner's decision of 2026-11-03, the QA report is a short **QA
section in the pull request body**, not a file under `docs/qa/`. The owner
reads it before deciding whether to build locally; it is written for that
reader.

The section contains:

- a table of acceptance criteria → tests or manual QA steps;
- a **Smoke list** line with the result of the
  [regression smoke list](#3-the-regression-smoke-list), the date and the
  platform;
- at most five lines on what was not verified;
- for a change to the interface, the 2–4 release captures the owner's
  acceptance quiz needs, at 960 × 540, plus 1600 × 1000 where layout matters.
  They are committed under `docs/qa/<yyyy-mm-dd>/<ticket>/` and linked from the
  pull request.

```markdown
## QA

| Acceptance criterion | Test / QA step |
| --- | --- |
| A dropped "Request" becomes a selected card with its inspector open | `palette_drop_adds_request` |
| The card lands at the drop point | `palette_drop_position` |

Captures (release binary): [960 × 540](docs/qa/2026-11-03/KTLO-05b/palette-drop-960.png),
[1600 × 1000](docs/qa/2026-11-03/KTLO-05b/palette-drop-1600.png)

Smoke list: 10 of 10 pass on the release binary, Linux, 2026-11-03.

Not verified:
- Keyboard-only placement of a block: no criterion, tracked as KTLO-11.
- Windows: not run, owner's build pending.
```

Rules:

- Captures come from the release binary of the branch, through `--screenshot`
  or the native host; a headless `egui_kittest` capture is a design review aid,
  not QA evidence.
- The verification commands, CI, the cubic and CodeQL checks before merge and
  the owner's acceptance stay as they are.
- A review finding about the wording of the QA section or of the ticket gets a
  one-line answer and never a commit of its own.
- Earlier reports under `docs/qa/` stay as they are.

### How to fetch Windows captures for a QA report

A CI run whose folder-project step completes uploads, per platform, an
artifact `qa-captures-<os>` (`qa-captures-windows-2022`,
`qa-captures-ubuntu-24.04`): the demo shop folder project captured by that
run's release binary in Code, Diagram and Split at 960 × 540 and
1600 × 1000, named `<mode>-<W>x<H>.png`. The artifact is uploaded even when the
step fails, so a failed run may hold fewer than six captures or none; confirm
all six are there. On Linux a capture of another size fails the step; on
Windows, whose display may be smaller than the window, it is only a warning on
the run, so check the run's warnings before citing a capture as a standard
size. Artifacts belong to one workflow run, so take them from the latest run of
the branch under review (Actions → the run for the branch's head commit →
Artifacts, or `gh run download <run-id> -n qa-captures-windows-2022`), cite the
commit of that run in the QA section of the pull request, and commit the
captures used under `docs/qa/<yyyy-mm-dd>/<ticket>/`. Other views are captured the same way with
`--screenshot OUT.png --folder PATH --mode … --size WxH`.

## 3. The regression smoke list

Run on the release binary before every merge to `main`, on Linux by the agent
and on Windows by the owner or from CI artifacts, and recorded in the
QA section of the pull request. The list is short on purpose; it covers the flows whose loss
would make the application unusable, and it grows only by a retrospective
action.

| # | Flow | Pass when |
| --- | --- | --- |
| 1 | Start with no arguments | The managed project opens; no error on stderr |
| 2 | Open a folder project | Explorer shows the tree; a scenario opens in Code |
| 3 | Add a block from the library panel | A card appears on the board and in the source |
| 4 | Wire two blocks | The wire routes around cards; source updates |
| 5 | Edit source, see the board | Board follows within 500 ms (the analysis pause is 250 ms, ADR 0007); invalid source holds the board |
| 6 | Run a simulation | Report shows every step; no request is sent |
| 7 | Save (Ctrl+S) and reopen | Exact bytes on disk; layout and identities restored |
| 8 | Close a dirty tab, then kill the process with unsaved work | Save / Discard / Cancel offered; recovery restores the work after a kill |
| 9 | Migrate a legacy workspace | `fixtures/workspace.json` (four projects) migrates and every project passes `check`; the original is byte-identical |
| 10 | Window at 960 × 540 | All controls visible and usable in Code, Diagram and Split |

The first Linux run is `docs/qa/2026-10-26/KTLO-07/report.md`; the owner's
Windows run is pending (KTLO-07, #10).

## Verification commands

The commands in the [development guide](../development.md#verification) run
before every pull request; interface changes also follow
[GUI testing](../gui-testing.md). Passing them is necessary, never sufficient:
acceptance is the owner's reading of the QA section.
