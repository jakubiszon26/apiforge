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
  An ignore with no ticket is a defect.

The whole suite still passes with the commands of the
[development guide](../development.md#verification).

## 2. The functional QA report

Every pull request that changes behaviour ends with a report at
`docs/qa/<yyyy-mm-dd>/<ticket>/report.md`, linked from the pull request. The
owner reads it before deciding whether to build locally; it is written for
that reader.

```markdown
# KTLO-05b: Block palette in the folder mode — QA report

- Branch and commit: …
- Built with: cargo build --release --locked
- Compared against mock-up: docs/tickets/KTLO-05a.md

## Linux
### Scenario 1: add a request from the palette
1. Open the demo shop project (`--folder …`).
2. Switch to Diagram (Ctrl+E).
3. Drag "Request" from the palette onto the board.
Expected: a new request card at the drop point, selected, with its inspector open.
Observed: as expected. [capture](palette-drop-960.png) at 960 × 540, [capture](palette-drop-1600.png) at 1600 × 1000.

### Scenario 2: …

## Windows
Same scenarios, with captures from the CI artifacts of this branch, or
"not run on Windows: <reason>" per scenario.

## Not verified
- Keyboard-only placement of a block: no acceptance criterion, tracked as KTLO-11.
- Screen reader session: not run.

## Regression smoke list
Result of each item of the list below, with the date and platform.
```

Rules:

- Captures come from the release binary of the branch, at 960 × 540 and
  1600 × 1000, through `--screenshot` or the native host; a headless
  `egui_kittest` capture is a design review aid, not QA evidence.
- "Observed" states what was seen, including what was wrong. A report with
  only expected outcomes and no "not verified" section is incomplete.
- Windows evidence comes from the CI artifacts of the branch (captures and the
  binary) and from the owner's local build; a report without a Windows
  section says so, and the ticket is not accepted for Windows.

### How to fetch Windows captures for a QA report

Every run of the CI workflow uploads, per platform, an artifact
`qa-captures-<os>` (`qa-captures-windows-2022`, `qa-captures-ubuntu-24.04`):
the demo shop folder project captured by that run's release binary in Code,
Diagram and Split at 960 × 540 and 1600 × 1000, named `<mode>-<W>x<H>.png`.
Artifacts belong to one workflow run, so take them from the latest run of the
branch under review (Actions → the run for the branch's head commit →
Artifacts, or `gh run download <run-id> -n qa-captures-windows-2022`), cite the
commit of that run in the report, and copy the captures used next to the
report. Other views are captured the same way with
`--screenshot OUT.png --folder PATH --mode … --size WxH`.

## 3. The regression smoke list

Run on the release binary before every merge to `main`, on Linux by the agent
and on Windows by the owner or from CI artifacts, and recorded at the end of
the QA report. The list is short on purpose; it covers the flows whose loss
would make the application unusable, and it grows only by a retrospective
action.

| # | Flow | Pass when |
| --- | --- | --- |
| 1 | Start with no arguments | The managed project opens; no error on stderr |
| 2 | Open a folder project | Explorer shows the tree; a scenario opens in Code |
| 3 | Add a block from the palette | A card appears on the board and in the source |
| 4 | Wire two blocks | The wire routes around cards; source updates |
| 5 | Edit source, see the board | Board follows within 250 ms; invalid source holds the board |
| 6 | Run a simulation | Report shows every step; no request is sent |
| 7 | Save (Ctrl+S) and reopen | Exact bytes on disk; layout and identities restored |
| 8 | Close with dirty tabs | Save / Discard / Cancel offered; recovery restores after a kill |
| 9 | Migrate a legacy workspace | The six-project fixture migrates; originals byte-identical |
| 10 | Window at 960 × 540 | All controls visible and usable in Code, Diagram and Split |

Item 3 fails on `main` at the time of writing; see KTLO-05.

## Verification commands

The commands in the [development guide](../development.md#verification) run
before every pull request; interface changes also follow
[GUI testing](../gui-testing.md). Passing them is necessary, never sufficient:
acceptance is the owner's reading of the report.
