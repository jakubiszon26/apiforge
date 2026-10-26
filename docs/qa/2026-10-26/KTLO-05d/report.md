# KTLO-05d: The Add step popup lists Variables and Operators below 1440 px — QA report

- Branch: `feat/ktlo-05d-popup-sections` (the PR head commit)
- Built with: `cargo build --release --locked -p apiforge-desktop` for the
  native captures; the debug test build for the harness captures.
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo test -p apiforge-desktop
  --locked` (340 passed, 17 ignored) and `apiforge-convert-v1 --fixtures
  --check`; output in the pull request.
- Compared against mock-up: `docs/tickets/KTLO-05a.md` (option A: the popup
  is the library in Split below 1440 px). The layout is the obvious one —
  the panel's two sections stacked under the popup's steps in its existing
  scroll area — so no layout question was put to the owner (see the ticket).

## Evidence

The popup cannot be opened from the command line, so the release-binary
captures show the board in Split without it, and the popup open comes from
the design review harness (`review_add_step_popup_in_split`, an
`egui_kittest` render of the same code), which is a review aid, not native
evidence.

| View | Before (main) | After |
| --- | --- | --- |
| Popup open, Split 960 × 540 | [before](before-popup-split-960x540.png) | [after](after-popup-split-960x540.png): the same steps at the top |
| Same, scrolled to the new sections | — | [after, scrolled](after-popup-split-960x540-scrolled.png): *VARIABLES* with *+* (*Add variable*) and the `baseUrl` chip, *OPERATORS* with Maths closed and Comparisons open |
| Popup open, Split 1600 × 1000 (panel in view) | [before](before-popup-split-1600x1000.png) | [after](after-popup-split-1600x1000.png): unchanged |
| Release binary, Split 960 × 540, no popup | — | [native](native-split-960x540.png) |
| Release binary, Split 1600 × 1000, no popup | — | [native](native-split-1600x1000.png) |

## Linux

| # | Criterion | Expected | Observed | Test |
| --- | --- | --- | --- | --- |
| 1 | Operators below 1440 px | In Split at 1439 px the popup offers the operator chips; *A = B* adds a Value block computing `A = B` at the board's visible centre, selected, with its statement in the source and the tab dirty | As expected (on the empty Cleanup board, so the centre is exact) | `narrow_split_popup_operator_chip_adds_like_the_panel` |
| 1 | Variables below 1440 px | *Add variable* in the popup adds a variable, selected, source updated, tab dirty; reopening the popup lists it as a variable chip | As expected | `narrow_split_popup_adds_and_lists_variables` |
| 1 | Fits 960 × 540 | The popup heading, *VARIABLES*, *Add variable*, *OPERATORS* and *A = B* are inside the window once scrolled to | As expected. Checked by hand as well: the last group header (*JSON data*) also reveals inside the window | `the_popup_sections_fit_the_minimum_window` |
| 2 | Unchanged with the panel | At 1440 px and 1600 × 1000 in Split, and in Diagram at 960 × 540, the popup has no Variables or Operators | As expected; the captures at 1600 × 1000 before and after look the same | `the_popup_is_unchanged_when_the_library_panel_is_in_view` |
| 3 | Accessibility | The folder audit passes with the popup open, in Split at 960 × 540 (with the sections) and 1600 × 1000 (without) | As expected after the change. On main the audit failed with the popup open: its step rows and search field had no accessible name. They are now named after the step (*HTTP request*, …) and *Find an action*; nothing visible changes | `the_add_step_popup_names_every_control` |

The tests were committed failing before the implementation (criteria 1 and 3
failed; criterion 2 passed on main, as an unchanged popup should).

## Windows

Not run; CI only (the desktop suite and the folder-project captures run on the
Windows runner).

## Not verified

- The popup open in the release binary: `--screenshot` cannot open it; the
  open popup is shown by the harness render only.
- Dragging a variable or operator chip out of the popup: chips keep the
  panel's drag payload, but no test or hand check covers a drag from the
  popup; not in the ticket's scope.
- Read-only board: the popup's new entries are not greyed out when the board
  is read-only (neither are its steps, as on main); the board ignores the
  edit, as it does for the steps.
- Windows, other than the CI suites.

## Noticed, not changed

At 960 × 540 the popup's lower edge sits at the window's edge, so its bottom
border is not drawn (the same on main; see the before capture). Everything in
it is reachable by scrolling.

## Regression smoke list (Linux, 2026-10-26)

| # | Flow | Result |
| --- | --- | --- |
| 3 | Add a block from the library panel | Pass in the harness: the KTLO-05b library tests pass unchanged (`folder/library_tests.rs`). Not repeated by hand on the release binary. |
| 10 | Window at 960 × 540 | Pass: [native capture](native-split-960x540.png) of Split; the popup at that size in the harness capture. |

The full list runs for KTLO-07.
