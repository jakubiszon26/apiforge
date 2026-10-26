# KTLO-05b: Block library beside the board (PR A) — QA report

- Branch: `feat/ktlo-05b-block-library` (the PR head commit)
- Built with: `cargo build --release -p apiforge-desktop --locked`
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, the desktop suite and
  `apiforge-convert-v1 --fixtures --check`; results in the pull request.
- Compared against the approved mock-up: `docs/tickets/KTLO-05a.md` (option A).

## Scope

PR A: the panel and its behaviour. Criterion 5's second half (the *Add step*
popup's Variables and Operators sections below 1440 px) is PR B. Criterion 6
(panel width persisted per project) waits for an owner decision on issue #8.

## Evidence

Behaviour is proven by the interaction tests in `folder/library_tests.rs`,
which drive the folder window through named widgets. Captures:

- **Release binary, native host** (Xvfb, `--screenshot … --folder`), Diagram
  and Split at the window's default 1600 × 1000, the mode restored from the
  project's workspace state: [Diagram](native-diagram-1600x1000.png),
  [Split](native-split-1600x1000.png). Observed: the panel sits between code
  and board in Split, with no *Examples* entry; with four columns the board
  fits the demo at 51 % zoom, so cards are small. The binary cannot yet be
  told a window size; `--mode`/`--size` come with KTLO-08 (PR #4).
- **Design review harness** (`tools/gui/review.sh`, a review aid only), before
  and after at both sizes: Diagram [before](before-diagram-1600x1000.png) /
  [after](after-diagram-1600x1000.png) at 1600 × 1000,
  [before](before-diagram-960x540.png) / [after](after-diagram-960x540.png) at
  960 × 540; Split [before](before-split-1600x1000.png) /
  [after](after-split-1600x1000.png).

## Linux

| # | Criterion | Expected | Observed | Test |
| --- | --- | --- | --- | --- |
| 1 | Panel and sections | *Block library* left of the board with search, Blocks / Variables / Operators and Project components; no *Examples*; every control named | As expected; the accessibility audit passes with the panel on screen | `diagram_shows_a_named_block_library_panel_with_its_sections` |
| 2 | Click a block | Added at the visible centre, selected, statement in the source, tab dirty | As expected, on an empty Cleanup board so the centre is exact | `clicking_a_block_adds_it_at_the_visible_centre_selected_and_dirty` |
| 3 | Drag a block | Added at the drop point | As expected | `dragging_a_block_onto_the_board_adds_it_at_the_drop_point` |
| 4 | Operators, variables | An operator chip adds a Value block computing that operator; Variables lists the document's variables; *Add variable* adds one | As expected | `an_operator_chip_adds_a_value_block`, `add_variable_adds_a_variable_node` |
| 5 | Split threshold (panel half) | Panel at 1440 px and wider, not at 1439 | As expected | `split_shows_the_library_only_when_wide_enough` |
| 7 | Read-only board | Panel disabled, no separate explanation | With an invalid source the blocks are disabled; the board's banner explains | `an_invalid_source_disables_the_library` |
| 8 | Minimum window | Panel, board toolbar and inspector inside 960 × 540 in Diagram; Split checked too | As expected in Diagram and Split at both sizes | `the_library_fits_the_minimum_window_in_diagram` |
| 9 | Empty-board hint | "Drag a block from the library or add the first step." only with the library in view | As expected | `the_empty_board_hint_depends_on_the_library_being_in_view` |

From review: the Cleanup board lists the main graph's variables
(`cleanup_lists_the_main_graphs_variables`), variable chips are named with
their kind ("total · variable"), and operator group headers report whether
they are expanded (`operator_group_headers_expose_their_expanded_state`).

## Windows

Not run. The owner's local Windows pass is pending; folder captures from CI
need KTLO-08 (PR #4).

## Not verified

- Criterion 6 (persistence): waits for the owner's decision on issue #8.
- Criterion 5, popup half: PR B.
- Read-only through a *held* source (recovered unsaved work): the same
  `editable` flag as the invalid case, not driven separately.
- Release-binary captures at 960 × 540 and of `main` (before): need `--size`
  from PR #4.
- Windows, and a screen-reader session.

## Regression smoke list (Linux, 2026-10-26)

| # | Flow | Result |
| --- | --- | --- |
| 3 | Add a block from the library panel | **Pass** — a click or a drag adds a card to the board and its statement to the source. Fails on `main`, which has no panel. |

The full list runs for KTLO-07.
