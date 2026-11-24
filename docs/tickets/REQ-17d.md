# REQ-17d: A try is kept — added to a scenario or saved as a new one

- Size: M · Owner review: 15 min · Risk: `visible UI`
- State: **Decided** ([REQ-17](REQ-17.md): 4 A). Mock-up approved
  (*Keep*, opened). After REQ-17b and REQ-01b; its third entry after
  REQ-18. Issue [#609](https://github.com/jakubiszon26/apiforge/issues/609).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

What a try found becomes part of a test in one choice; a name is asked
only then.

## What the owner will see

- New: *Keep ▾* beside *Send*: *Add to a scenario…*, *Save as a new
  scenario…*, *Save as component…*, and *Copy as cURL* once REQ-07
  exists.

## Acceptance criteria

Tests in `folder/acceptance_tests/req17d.rs`.

1. *Add to a scenario…* lists the project's scenarios by title
   (WS-05b); the request lands after that scenario's selected step, or
   last in Main, by DIAG-22's operation; the scenario's tab opens with
   the step selected. `req17d_criterion_1_*`
2. *Save as a new scenario…* asks a name as *New scenario* does and
   registers it (`attach`, REQ-17a); the tab becomes the scenario's.
   `req17d_criterion_2_*`
3. What is written is what building that request in the window writes
   (REQ-13): no line the person did not set. `req17d_criterion_3_*`
4. A handle taken in the scenario is made unique as a pasted step's is.
   `req17d_criterion_4_*`
5. After *Add to a scenario…* the try stays; after *Save as a new
   scenario…* it is no longer a try. `req17d_criterion_5_*`
6. *Save as component…* opens REQ-18's dialog for the try; shown only
   when REQ-18 is merged. `req17d_criterion_6_*`
7. At 960 × 540 the menu stays inside the window. `assert_within_window`

## Technical appendix

- Files: `folder/tries.rs`, new `folder/keep.rs`; reuse
  `board_edit` (DIAG-22's insert), `file_prompts.rs` (the name);
  the test file.
- Regression: DIAG-22's and WS-05b's tests unchanged.
