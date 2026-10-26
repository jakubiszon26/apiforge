# KTLO-05c: The block library panel's width is remembered per project — QA report

- Branch: `feat/ktlo-05c-library-width` (the PR head commit)
- Built with: `cargo build -p apiforge-desktop --locked` (debug, through the
  test suites); the release binary was not built for this report.
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, the storage and desktop suites and
  `apiforge-convert-v1 --fixtures --check`; output in the pull request.
- Compared against mock-up: not applicable (no visible layout change).

## Evidence

No captures: the change has no visible layout change; the panel looks as in
KTLO-05b and only its width is kept. Behaviour is proven by tests: criteria
1–3 against the project store in `apiforge-storage`
(`project/workspace.rs`), criterion 4 as an interaction test that drags the
panel's named resize handle (`folder/library_tests.rs`). All four were
committed failing before the implementation.

## Linux

| # | Criterion | Expected | Observed | Test |
| --- | --- | --- | --- | --- |
| 1 | `panels.library` round-trips | Loaded, saved again with the same value (287.5) | As expected | `the_library_panel_width_round_trips` |
| 2 | No `panels.library` | Loads; the library gets the default width 240 (the panel's former fixed default) | As expected; the other sizes keep their values | `a_view_state_without_a_library_width_loads_with_the_default` |
| 3 | Unknown panel size | Loads; the unknown size is dropped on the next save, the known sizes keep their values; unknown fields elsewhere in the view state are still rejected | As expected. The manifest's own rejection of unknown fields is covered by the existing `schema_rejects_unknown_versions_fields_and_collisions` | `an_unknown_panel_size_is_dropped_and_the_known_ones_kept` |
| 4 | Resize, close, reopen | Dragging *Resize block library* 60 px right widens the panel; after closing the window and opening the project again, the panel's edge is where it was left | As expected (handle at 248 px at the default width, 306 px after the drag and again after reopening) | `the_library_width_comes_back_after_reopening_the_project` |

Before this change, on `main`: a view state with a `library` size, or any
unknown panel size, was refused as a whole, so the project opened with the
default mode, tabs and panel sizes; the library always opened at 240 px.

## Windows

Not run; CI only (the storage and desktop suites run on the Windows runner).

## Not verified

- Release binary at the window: the width was not checked by hand on the
  release build; criterion 4 is covered by the interaction test only.
- An older build reading a file written by this build: builds before this one
  still reject the `library` size and fall back to the default view; only
  builds from this one on ignore unknown panel sizes.
- Windows, other than the CI suites.

## Regression smoke list (Linux, 2026-10-26)

| # | Flow | Result |
| --- | --- | --- |
| 7 | Save (Ctrl+S) and reopen | Not run on the release binary. The view state's round trip is covered by the existing `the_view_and_unsaved_work_come_back_after_closing_the_window` and by criterion 4's test, both passing. |

The full list runs for KTLO-07.
