# UX-01: Several checks from one run — QA report

- Branch: `docs/ux-01-several-checks` (pull request #243). Captures are from
  the release binary of `22bbf2d`. Two code commits came later:
  `e702e31` keeps the comparison of the text with the run's source for its
  source version instead of making it each frame (nothing drawn changed),
  and review round 1, `7f71bd1`, changes the rows of a document that
  cannot be edited only (see *Review round 1*).
- Built with: `cargo build --release --locked -p apiforge-desktop`
  (Rust 1.99.0).
- Compared against mock-up: the approved UX-00 captures `ux01-*`
  (`mock/ux-00` at `2c1fa79`), copied to [`../UX-00/`](../UX-00/); the
  ticket is [`docs/tickets/UX-01.md`](../../../tickets/UX-01.md).
- Analysis model: Fable. Implementation model: Claude Opus 5.5.
- Acceptance tests first: commit `c462e12`. Before the implementation all
  eight `ux01_criterion_*` tests failed for the right reason: the first
  *Check* made the results stale, so the bar read *Graph changed*, no field
  offered a second *Check*, and the board lost the run's marks. One
  assertion changed after that commit, in `9182597`, before the
  implementation: criterion 8 asserted that every *Check added* row is
  inside the window, which a row scrolled below the restored panel at
  960 × 540 is not (its accessibility node keeps a place there, nothing is
  painted). It now checks the rows where they show: maximised at 960 × 540
  and at the tallest normal height at 1600 × 1000.
- Two refinements of criterion 2 in the analysis described today's
  behaviour wrongly; the tests keep today's behaviour and the ticket says so
  (*Corrections found writing the tests*, `2fc0262`). See *Deviations*.
- Verification: at the end of this report.

## What changed

Each run's report keeps the checks added from it, each keyed by a hash of
the scenario's text right after the check's edit (`DefaultHasher`, never
saved). The report is *fresh*, *checks only* (the first *n* checks are the
only change) or *stale*. While it is checks only, the bar says how many
checks wait, the checked rows read *Check added*, the error policy and the
other *Check* buttons stay usable, and the board and the code keep the
run's marks. Any other change makes it stale exactly as before. Undo and
redo need no bookkeeping: the text returns to the text after an earlier
check, or to the run's own source. The next run starts a new report and a
new count. `ProjectSession::run_is_stale_beyond_text` tells a change of the
text from every other reason for stale results (another session or
context, unresolved fields, a conflict, a changed registry, a run of the
last valid version).

## Criteria and their evidence

Tests are in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`,
`mod ux01`, on the demo shop in Split at 960 × 540 unless said otherwise.

| # | Criterion | Test | Release binary |
| --- | --- | --- | --- |
| 1 | *Check* on `productId` adds a card, the panel stays on *Add to cart*'s Outputs, the row reads *Check added*; *Check* on `total` adds a second card without a run | `ux01_criterion_1_two_checks_from_one_run` | Scenario 1 |
| 2 | The bar counts the checks in grey; another change gives *Graph changed* in amber, no count, no *Check* | `ux01_criterion_2_the_bar_counts_the_checks`, `ux01_criterion_2_another_change_makes_the_results_stale` | Scenarios 1 and 3 |
| 3 | Ctrl+Z outside the code takes the last check back; a second gives fresh results; redo brings one back | `ux01_criterion_3_undo_takes_a_check_back` | Scenario 2 |
| 4 | The result matches `ux01-*` at 960 × 540 and 1600 × 1000 | this report | Scenario 1, *Against the mock-up* |
| 5 | The next run runs the checks and starts a new count | `ux01_criterion_5_the_next_run_runs_the_checks` | Scenario 4 |
| 6 | The board and the code keep the run's marks while only checks were added | `ux01_criterion_6_the_board_keeps_the_run` | not distinguishable in the captures: the board is at 25–35 % zoom |
| 7 | A component's own run counts its checks the same way | `ux01_criterion_7_a_component_run_counts_its_checks` | not run (see *Not verified*) |
| 8 | The bar fits at both sizes; a *Check added* row stays in the window | `ux01_criterion_8_the_bar_fits` | Scenario 1 |
| 9 | Gestures and states named in every test | the tests above | — |

The design review view `review_ux01_two_checks` (`testing::review`,
`tools/gui/review.sh review_ux01`) renders the five mock-up states against
the real controls. It replaces the mock-up's `mock_ux00` switches, which
are not merged. It was rendered on `22bbf2d` and its captures were looked
at; they show what the release captures below show.

## How the release binary was driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
scratch configuration, data and runtime directories. Clicks and keys went
through the X test extension (libXtst from the Ubuntu archive, unpacked
into a scratch directory, not installed); the screen was grabbed from the
display. The 960 × 540 window is the same process resized through Xlib;
1600 × 1000 is the window's own first size. The project is a fresh
`prepare_project` of the demo shop (`apiforge_core::demos::shop_project`),
the same one the tests use, in a scratch directory; each scenario below
that says *fresh* starts the window again on a new copy.

## Linux

### Scenario 1 (criteria 1, 2, 4, 8): two checks from one run

1. Open the shop (fresh); 960 × 540; *Split*; *Run*; dismiss *Running saved
   code.*; click the *Add to cart* card on the board; *Maximise run
   results*.
2. *Check* on the `productId` row.
3. *Check* on the `total` row.
4. *Restore run results*.
5. Fresh window at 1600 × 1000: *Split*; *Run*; dismiss the notice; click
   *Add to cart*; click the panel's edge and press End (its tallest normal
   height, 560 px); click an empty part of the panel; steps 2 and 3.

Expected: every field offers *Check* and *To variable*; after step 2 the
`productId` row reads *Check added* with a green tick and the bar *1 check
added · it runs on the next run* in grey; after step 3 the `total` row too
and *2 checks added · they run on the next run*; the other rows unchanged;
*Continue remaining steps* enabled; the bar's text fits left of the
maximise button at both heights and both sizes; the new cards on the board
and the code beside the count.

Observed: as expected.
[before](ux01-before-960x540.png),
[one check](ux01-one-check-960x540.png),
[two checks](ux01-two-checks-960x540.png),
[restored](ux01-two-checks-restored-960x540.png) at 960 × 540: in the
restored capture the code shows `check_productid = assert
add_to_cart.productId == 123` and `check_total = assert add_to_cart.total
== 80`, the board the cards *Check productId* and *Check total* (the latter
selected), the bar's text left of the maximise button and clear of
*PASSED*, as in the mock-up. At 1600 × 1000:
[before](ux01-before-1600x1000.png),
[two checks](ux01-two-checks-1600x1000.png), the Inspector showing the new
*Check total* card. The panel stayed on *Add to cart* throughout.

### Scenario 2 (criterion 3): undo and redo

1. As scenario 1 steps 1–3 (fresh, 960 × 540, maximised).
2. Ctrl+Z (the keyboard is not in the code editor).
3. Ctrl+Z.
4. Ctrl+Shift+Z.

Expected: after step 2 *1 check added* and `total` offers *Check* again;
after step 3 no bar text and both rows offer *Check*; after step 4 *1
check added* and `productId` reads *Check added*.

Observed: as expected: [one undo](ux01-undo-one-960x540.png),
[two undos](ux01-undo-two-960x540.png) (*Save* and *Save all* disabled
again: the text is the saved one),
[redo](ux01-redo-960x540.png).

### Scenario 3 (criterion 2): another change

1. Continuing scenario 2: *Check* on `total` again (two checks), *Restore
   run results*.
2. Click at the end of line 19 of the code and type a space.
3. *Maximise run results*.

Expected: *Graph changed · results from an earlier version* in amber, no
count, the policy combo disabled, no field offering *Check* or *To
variable*.

Observed: as expected: [restored](ux01-stale-restored-960x540.png),
[maximised](ux01-stale-960x540.png).

### Scenario 4 (criterion 5): the next run

1. Continuing scenario 3: *Run*.

Expected: the run includes the two checks; no bar text.

Observed: *10 passed* where the first run had *8 passed*, *Running unsaved
code.*, no bar text; the panel shows the selected card's step, *Check
total*, passed with expected and actual 80
[capture](ux01-next-run-960x540.png). That the fields of *Add to cart*
offer *Check* again after the run is covered by the test, not captured.

### Against the mock-up

| | `ux01-*` (mock-up) | This branch |
| --- | --- | --- |
| Checked row | tick and *Check added* in green where *Check* and *To variable* were | the same |
| Other rows | *Check* and *To variable* | the same |
| Bar, two checks | *2 checks added · they run on the next run*, grey, left of the maximise button | the same, at both sizes and heights |
| Bar, one check | not drawn | *1 check added · it runs on the next run* (the ticket's singular) |
| Header | *Run simulation* in blue and *Live allowed* (other tickets' mock-ups, UX-02 and MCP-09's consent) | *Run* as on `main`; no consent ticked in the scratch configuration |
| Step list | not scrolled; *Add to cart* chosen on the board | the same at 1600 × 1000 and in the 960 × 540 captures, which choose the card on the board as the mock-up did |
| Error policy | enabled | enabled |

No other visible difference. The mock-up's pixels are the headless
harness's (an 8 px margin around the window); the release window has none,
so positions differ by that margin.

## Deviations

- **A moved block keeps the count.** The ticket's criterion 2 refined
  *another change* to include moving a block, *as today*. Today a move
  does not make results stale: it changes a card's position, not the source
  (measured on `main` at `115456f`). The implementation keeps that, so a
  move after checks keeps *N checks added*; the board case of the test is a
  block added with *Add step* instead, and the test asserts that a move
  keeps the count. Making a move stale would end the checks the first time
  a new check card is moved out of the way. Recorded in the ticket.
- **Stale results offer no *Check*.** The ticket says the remaining
  *Check* buttons are disabled *as today*; today they are not shown at all
  on stale results, and the implementation keeps that.
- **A check from the Inspector counts too.** The Inspector's *Check* on a
  card's output data (shown while the results are fresh) adds the same
  assertion through the same path; it is counted like one from Run
  results. Not separately tested.

## Windows

Not run on Windows by hand. The CI run of this branch on `windows-2022`
runs the `ux01_*` acceptance tests: run 41509709984 on `22bbf2d`, all eight
passed (`Build & Test (windows-2022)`, success). Its `qa-captures`
artifact has the standard mode captures only, not the UX-01 states. The
ticket is not accepted for Windows on this report.

## Not verified

- Criterion 7 (a component's own run) in the release binary; its test
  covers it.
- Criterion 6 in the release binary: the cards' pass marks are too small
  to tell apart at the board's zoom in these captures; the test covers it.
- A moved block after checks, in the release binary; the test covers it.
- The fields offering *Check* again after the next run, in the release
  binary (scenario 4 shows the run, not the fields); the test covers it.
- A check whose edit first asks *Continue without unattached comments?*:
  the check is applied after the prompt and is not counted, so the results
  read *Graph changed* as before this change. The demo has no such
  comments; no test.
- Double scale, keyboard-only operation and a screen reader session.
- Windows, beyond the CI test run.

## Review round 1

The automated review of `950e007` (cubic, eight points) is answered on the
pull request. Fixed:

- A checked row reads *Check added* also while the document cannot be
  edited (another process holds the project, or it is read-only); such a
  row, like every row then, offers neither *Check* nor *To variable*. New
  unit test `report::tests::a_checked_row_reads_check_added_while_the_document_is_locked`.
  Not captured in the release binary.
- The ticket's *What the owner will see* and criteria 2 and 4 now match the
  tests and this report (a moved block, stale rows without *Check*, Windows
  captures not compared), and name the controls a checked row gives way to.
- The nearest-*Check* helper is shared by the tests and the review view
  (`testing::check_field`).
- The legacy workspace window says in code that it keeps its behaviour.
- The smoke line reports both runs' totals.

## Regression smoke list

Linux, 2026-11-02, release binary of `22bbf2d`:

| # | Result |
| --- | --- |
| 6 | Run a simulation: the first report shows every step, *8 passed*; the next run, with the two checks, *10 passed*; no request sent (scenarios 1 and 4) |
| 2, 10 | CI run 41509709984 on `22bbf2d`, `qa-captures-ubuntu-24.04`: the migrated demo shop in Code, Diagram and Split at 960 × 540 and 1600 × 1000, all six at the requested size, the tree, the editor and the board shown, every control visible. On `windows-2022` the 960 × 540 captures are the requested size and the 1600 × 1000 ones are 1024 × 768, the runner's display, as on earlier runs (not committed) |
| 1, 3–5, 7–9 | Not run on the release binary for this ticket; the workspace suite covers their tests |

The owner's Windows run is pending as for the earlier tickets.

## Verification

On `7f71bd1` (review round 1), all exit 0: `cargo fmt --all -- --check`;
`cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo
test --workspace --all-targets --locked`: 1 431 passed, 0 failed, 44
ignored; `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`.
Earlier: `e702e31` and `22bbf2d`, 1 430 passed, 44 ignored. CI on
`22bbf2d` (runs 41509709984 and 41509716347): Linux and Windows build and
test, CodeQL, all green.
