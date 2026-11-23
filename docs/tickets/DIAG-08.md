# DIAG-08: A check of a field's type in one step from the field

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: **Approved, ready for implementation** (issue #521) · decided
  by the owner on 2026-11-21 on the mock-up: **A, the ✓ opens a menu**;
  the window uses **the language's type words**; #399 goes into 0.2.1 as
  DIAG-09, **which merges first** · for 0.2.1 · found by the owner on the
  v0.2.0-rc.1 packages, 2026-11-21 · labelled `ready:opus`
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

"It is hard to check a field by asking whether it is the right data
type." From a field of a response — `total` — the person gets the check
*total is a number* without knowing where the type check hides.

## What is there today

The check exists: `assert cart.body.total is "number"` in the language,
*Has type* in the Inspector. The way to it is the difficulty:

1. Output data → the ✓ on the field's row (*Add comparison*) always
   writes *equals 172.5*. Before the first run it writes nothing (#399).
2. Select the new check → *Check*, a list of nine → *Has type*, the
   eighth.
3. *Expected* turns into a list that says *Text, Number, Yes / no,
   Object, List, Null*, while the file, the check's line on the card and
   Run results say `string`, `number`, `boolean`, `array`.

Three places and two vocabularies for one sentence. In the code the type
is a quoted word, and a misspelt one is not reported: LANG-22.

## What the owner will see

- Changed: the ✓ on a row of Output data (the Inspector, and the same
  tree in Run results) opens the menu *Add a check* (decided: A).
- New: the checks *is a ‹type›*, *exists* and *is not empty* written
  from a field, beside *equals ‹value›*; each is an ordinary check, on
  the board and in the file, as *Add comparison* writes one today.
- Changed (decided): the window says `string`, `number`, `boolean`,
  `object`, `array`, `null` wherever it names a type of a value; the
  Inspector's *Text, Number, Yes / no, Object, List, Null* go.
- Removed or hidden: nothing. Default behaviour that changes: in A the ✓
  opens a menu instead of writing *equals* at once — one more click for
  the check it writes today.

## Mock-up

Design review capture, `review_rel021_type_check` on `mock/rel-0.2.1` at
`58710bc`: `docs/qa/2026-11-21/DIAG-08/type-check.png` (the Inspector of
`cart` after a run, 1600 × 1000; today, A, B).

Decided by the owner on 2026-11-21, recorded on #521: **A**. The table
keeps what was offered.

| Option | Look | For the person at the window |
| --- | --- | --- |
| **A — the ✓ opens a menu (chosen)** | *Add a check*: *total equals 172.5*, *total is a number*, *total exists*, *total is not empty* | Every common check of a field in two clicks, read as a sentence before it is written; nothing new on the rows |
| B — a type chip on every row | `number` beside the value; a click writes *is a number*; the ✓ stays *equals* | One click, and the type of every field is in view; a third control on each row of a narrow panel, and *exists* stays three places away |

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/diag08.rs`
and, for the edit, `crates/apiforge-core/tests/acceptance.rs`.

1. From the row `total` of a response, *is a number* adds one check; the
   file gains `… = assert cart.body.total is "number"` under the step and
   its checks, and `check` passes. `diag08_criterion_1_type_check_from_a_field`.
2. The type offered is the type of the value shown: `string`, `number`,
   `boolean`, `object`, `array`, `null`. `diag08_criterion_2_the_type_is_the_values`.
3. *exists* and *is not empty* write `assert exists …` and
   `assert nonempty …`; *equals* writes what *Add comparison* writes
   today. `diag08_criterion_3_the_other_checks`.
4. Each added check is one undo step and appears on the card's checks
   line. `diag08_criterion_4_one_undo_step`.
5. The Inspector's *Expected* list of a *Has type* check, the type list
   of a literal, the menu, the card's line and Run results use the
   language's six words.
   `diag08_criterion_5_one_vocabulary`.
6. The same menu on the same tree in Run results.
   `diag08_criterion_6_run_results`.
7. Before the first run the menu writes the same checks from the
   step's sample (on DIAG-09's path). `diag08_criterion_7_before_a_run`.
8. Design review at 960 × 540 and 1600 × 1000 with the menu open.

## Not in scope

Making the ✓ work before the first run: DIAG-09 (#399), merged before
this ticket. One check for several
fields and types written as a shape (LANG-19 #472). Checks ticked from a
whole response (REQ-10 #478). The diagnostics of a type check: LANG-22.

## Proposals beyond the request

| Proposal | Problem solved | Size, risk | Trade-offs | Where |
| --- | --- | --- | --- | --- |
| A row of Output data on one line: name, value, actions | Each field takes two lines with its value centred under it; a response of ten fields does not fit the panel | S · `visible UI` | Long values are cut sooner | REQ-02a (#458), which redraws the response |
| A type in LANG-19's `expect`: `expect cart { body: { total: number } }` | One line states the shape of a response; no check per field | part of LANG-19 · `format` | Decided with LANG-19's analysis, not here | LANG, second half |

## Technical appendix

`inspector/fields.rs` `tree_row` and `TreeAction`; `inspector/mod.rs`
`Action::AssertField`; `report.rs` `Action::AssertField`;
`apiforge_core::edit::field_assertion` gains the predicate;
`fields::assertion_type_combo` holds the second vocabulary. The menu is
one function called from the row, so that DIAG-10, which redraws the
row after this ticket, moves the call and nothing else. The mock
branch has both variants behind `mock::TYPE_CHECK`.
