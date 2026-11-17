# LAY-19: A failure shows its consequences — not-run steps by cause

- Size: M · Owner review: 20 min · Risk: `visible UI`
- State: Analysis (issue #303) · re-checked against `main` at `0677254` on
  2026-11-17, after RUN-03 (#409); the design-review view is posted to the
  issue and waits for the owner's answers (below). Added to the sprint by
  the owner on 2026-11-05 (Q12 = C; the second opinion's item 6 of
  [lay.md 4.5](../sprints/lay.md#45-second-opinion-part-two-local-context);
  runner-side, kept in LAY by that decision) · LAY sprint, part B; its
  runner half, LAY-22 (#312), is merged
- Analysis model: Fable · Implementation model: Opus

## Goal

After a failed run the board says what the failure cost: the card that
holds the failure counts the steps it kept from running, and every step
that did not run names its cause — stopped, blocked, or skipped — and the
failure behind it, never one grey "Skipped · 0 ms".

## What was found on `main` (2026-11-17)

Read in the running window and in the run's results, on `shop-30`:

1. **The reference failure is a check.** With `checkout`'s sample total
   changed, `checkout_total` fails and eleven steps do not run. Checks are
   collapsed by default (LAY-11), so the failed step has no card of its
   own: it is folded into `checkout`, whose checks line reads
   `19–21 · ✕ 1/3 failed · 2 skipped`. The count therefore sits on the
   card that holds the failure — the failed step's own card, or the
   subject of a folded check.
2. **The engine names the nearest step, not the failure.** Of the eleven,
   three carry `Stopped { by: checkout_total }`; the other eight carry
   `BlockedBy` the step before them (`paid` is blocked by `order`,
   `payment` by `paid`, …) with the message "Step outside the active
   execution path." Read literally, eight cards of a stopped run would say
   *Blocked: paid*. The window follows each cause to its end instead: the
   step that ran and failed, and the kind of the last link (stopped,
   blocked, condition). The engine is not changed.
3. **Today's cards**: a not-run card reads `Skipped · 0 ms` or
   `Blocked · 0 ms`; Run results' summary reads `20 passed 1 failed` and
   leaves the eleven out; a not-run row reads `Skipped`.
4. **A Value card and an IF card have no result line** (LAY-16). They
   show a failure or a step that did not run by their band and border
   only.
5. **At Fit a card has no footer** (LAY-09). At 29 % (1600 × 1000) a
   second line has room on the failed card; at 17 % (960 × 540, Run
   results collapsed) no line of 9 px has room on any card.
6. **A false *Data condition*** ran: its status is `Blocked` and it has
   no cause. The steps behind it carry `ConditionUnmet`, and the steps
   behind those `BlockedBy`.
7. `NotRunCause` has a sixth variant, a Cleanup step whose input is
   missing.

## What the owner will see

- New, on the card that holds a failure: `11 steps not run`, bold, in the
  failure's red, at the right end of the result line; at Fit `11 not run`
  under the name where the card has room. A false *Data condition* that
  kept steps from running carries the same count in amber.
- New, in Run results: the summary counts every step (`20 passed ·
  1 failed · 11 stopped`; also `blocked`, `skipped`, `cancelled`,
  `not met`); a not-run row reads its cause; the failed row reads
  `Failed · 11 steps not run`; a not-run step's details say why in one
  sentence.
- Changed: the result line of a card that did not run (option A, the
  recommendation; option B writes the first word only):

  | The step… | Card and row |
  | --- | --- |
  | did not run because the policy *Stop* ended the run | `Stopped · checkout_total failed` |
  | needs a step that failed, or a step that did not run because of it | `Blocked · refund failed` |
  | is behind a false *Data condition* | `Blocked · valid_cart unmet` |
  | is on the branch not taken | `Skipped · outside path` |
  | was not reached before the person stopped the run | `Cancelled` |
  | is a Cleanup step whose input was never produced | `Skipped · no value from cart` |

  The `· 0 ms` of a step that did not run is no longer written. A false
  *Data condition* reads `Not met` where it read `Blocked`.
- Removed or hidden: the engine's sentence "Step outside the active
  execution path." in a not-run step's details, replaced by the sentence
  above. Default behaviour that changes: none. Migrated: nothing.
- Not changed: a Value card and an IF card (finding 4); the colours
  (LAY-09); cards at Fit that did not run keep their grey band without a
  word (finding 5).

## Mock-up

Design-review view `review_lay19` on `mock/lay-19-failure-consequences`;
captures in [docs/qa/2026-11-17/LAY-19](../qa/2026-11-17/LAY-19/), posted
to the issue. `shop-30`, tidied, checks collapsed, after a simulation.
In every capture the list of Run results is scrolled to the chosen step,
which the window does not do today.

| Capture | Shows |
| --- | --- |
| `lay19-before-stop-60-1600x1000`, `…-960x540` | `main`: the failed check under *Stop*, at 60 % |
| `lay19-a-stop-60-1600x1000`, `…-960x540` | option A, the same view |
| `lay19-b-stop-60-1600x1000` | option B: the word alone |
| `lay19-a-stop-fit-1600x1000`, `…-960x540` | Fit: the count at 29 %; nothing new at 17 % |
| `lay19-before-stop-results-1600x1000`, `lay19-a-stop-results-1600x1000` | Run results maximised, `order` chosen |
| `lay19-a-continue-error-60-1600x1000` | *Continue*: a request with no sample; blocked, skipped |
| `lay19-a-continue-condition-60-1600x1000` | *Continue*: a false *Data condition* |
| `lay19-a-value-before-start-60-1600x1000` | the run of #418: a value fails before *Start* |

## Questions for the owner

1. **The words on a step that did not run.** A (recommended): the cause
   and the failure, `Stopped · checkout_total failed`. B: the cause
   alone, `Stopped`.
2. **What the count counts.** A (recommended): the rows of Run results,
   checks included — eleven here, the summary's `11 stopped`. B: the cards
   on the board — six here with checks collapsed, eleven with checks drawn.
3. **Value and IF cards.** A (recommended): unchanged in this ticket;
   their words are in Run results. B: a new ticket gives them a result
   line when they failed or did not run, which makes those cards taller
   after a run.
4. **A false *Data condition*.** A (recommended): it reads `Not met`,
   carries its own count in amber, and the summary says `1 not met`.
   B: it stays `Blocked` with no count.

## Acceptance criteria

Tests in `acceptance_tests`, `mod lay19`, on `shop-30.afs`.

1. After the failed simulation under *Stop* (`checkout`'s sample total
   changed), the card that holds the failure — `checkout` with checks
   collapsed, `checkout_total` with checks drawn — reads the number of
   steps whose cause ends at `checkout_total`: eleven. A step behind an
   unrelated false *Data condition* is not counted. Test:
   `lay19_criterion_1_the_count_equals_the_caused_rows`.
2. Each card that did not run and has a result line reads the kind of the
   last link of its cause and the step at its end: all of the eleven read
   *Stopped · checkout_total failed*, although eight carry `BlockedBy`.
   Under *Continue* with a request that has no sample, its readers and
   theirs read *Blocked · <request> failed*; behind a false *Data
   condition*, *Blocked · <condition> unmet*. Test:
   `lay19_criterion_2_each_card_names_the_end_of_its_cause`.
3. A step on the branch not taken reads *Skipped · outside path*, never
   *Blocked*, in a passing and in a failing run. Test:
   `lay19_criterion_3_a_branch_not_taken_is_skipped`.
4. Run results' summary counts add up to the number of rows, and each
   not-run row reads what its card reads; the failed row reads its count.
   Test: `lay19_criterion_4_the_summary_agrees`.
5. At Compact and at Full the count and the cause are on the result line
   and inside LAY-16's box; the count is whole, and a cause too long for
   the line ends in an ellipsis after its first word. At Overview the
   count is drawn only where 9 px text has room (LAY-09's rule), and a
   card that did not run has no word. Checked at 1600 × 1000 and
   960 × 540. Test: `lay19_criterion_5_fits_the_box`.
6. The window reads `NotRunCause` and never a message string: with every
   `message` removed from the run, the cards and Run results read the
   same. Test: `lay19_criterion_6_no_message_is_read`.
7. A run cancelled by the person marks the steps not reached *Cancelled*,
   not *Stopped*, and no card counts them. Test:
   `lay19_criterion_7_cancelled_is_its_own_cause`.
8. A value that fails before *Start* (the run of #418) is the end of
   every cause: the cards read *Stopped · <value> failed* and Run results
   count them as stopped, with the engine as it is today. Test:
   `lay19_criterion_8_a_failure_before_start`.

## Not in scope

- Changing when the engine stops, blocks or skips, the cause it records
  or the message it writes. The order of a value and *Start*, and the
  message "Step outside the active execution path." on steps after a
  stop, belong to RUN-04 (#418); the command line and the MCP `run`
  result keep that message until then. LAY-19 neither waits for #418 nor
  conflicts with it: the window follows the cause to its end either way,
  and criterion 8 holds before and after.
- A result line on Value and IF cards (question 3).
- A Blocked step's retry; the colours of the statuses (LAY-09).

## Technical appendix

- `StepResult::cause` and `NotRunCause::step()` exist (LAY-22). The
  chain walk of `crates/apiforge-runner/tests/acceptance.rs` (`chain_end`)
  is the rule; it moves to `apiforge-core` beside `NotRunCause` so that
  the window and a later command-line summary share it: for a step, the
  step at the end of its cause and the kind of the last link; for a run,
  the count by end.
- `crates/apiforge-desktop/src/canvas/paint.rs`: `CardRun::line` (the
  result line at Compact and Full), the Overview name block of
  `compact_card`; `canvas/mod.rs` gives each card its words and the count
  of its own step plus its folded checks (`fold` knows the subject).
- `crates/apiforge-desktop/src/report.rs`: `header_row` (summary),
  `step_row_button` (row), `step_details` (pill and sentence).
- The mock-up's code (`consequence.rs` and the `LAY19` switch) is a
  sketch on the mock branch and is not merged.
