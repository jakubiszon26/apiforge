# WS-05a: A scenario's title is shown above its board and edited there

- Size: M · Owner review: 15 min · Risk: `visible UI`
- State: **Approved, ready for implementation** (issue #574) · decided by
  the owner on 2026-11-24 on the board, recorded on #574: **B** (*Files*
  by title, the header as in A) and **the title is edited in the
  header** · split on 2026-11-24: this ticket is the header and the
  edit, [WS-05b](WS-05b.md) (#596) is *Files* · the same on v0.1.0 and
  v0.2.1-rc.2, not a regression · labelled `ready:opus` · REQ milestone
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A tester knows which test is open by its title, and renames it, without
opening Code.

## Finding

Looked at on `main` (`1923fcd3`). The header above the board is
`folder/diagram.rs`, `Header` and `HeaderFit` (the file name and the
kind). The title is `Scenario::name` (a component's `name`) of the
document's model, which the session holds for every registered
document. No owner question is open.

## What the owner will see

- Changed: the header reads the title in bold, then the file name and
  the kind in grey. A double click on the title makes it a field.
- Removed or hidden: nothing. As the row narrows the file name gives way
  before the title is cut; it stays in the tab above and on hover.

## Mock-up

Approved as a whole on 2026-11-24: `docs/qa/2026-11-24/WS-05/title.png`
(the header of columns A and B is this ticket), `hover-edit.png` (the
edited title), whole windows `a-960.png`, `a-1600.png`. From
`review_ws05_title` on `mock/ws-05-req-15` at `aa6518b6`; the mock's
code is a drawing, not a start for the implementation.

## Acceptance criteria

Tests in `folder/acceptance_tests/ws05a.rs`; the fixture is a project of
three scenarios, one with a long title and a long file name, kept in the
test (WS-05b uses it).

1. In Diagram the header shows the scenario's title, then the file name,
   then the kind; a component shows its name the same way.
   `header_shows_title`.
2. As the row narrows the kind goes first, then the file name (whole or
   not at all), then the title ends in an ellipsis and never goes below
   today's least width of the name. At 960 × 540 with the long fixture,
   with and without the side panel, the title, the tabs and the toolbar
   are disjoint (`assert_disjoint`). `header_gives_way_in_order`;
   `header_tests.rs` stays green, extended and not replaced.
3. Hover on the title reads the title, then the project-relative path.
   `header_hover`.
4. A document with no valid model keeps today's header, the file name in
   bold; a title that is empty in the code does the same.
   `header_without_a_title`.
5. The title follows an edit in Code once the analysis has run, saved or
   not. `title_follows_the_code`.
6. A double click on the title opens a field holding the whole title,
   selected, with the keyboard in it. Return writes the title: the
   `scenario "…"` (or `component "…"`) line changes and nothing else in
   the text, the document is dirty, one undo step brings the old title
   back. `title_is_edited_in_the_header`.
7. Escape, and an unchanged or empty text however the field is left,
   write nothing: not dirty, no undo step. `title_edit_is_abandoned`.
   Anything else that leaves the field — a click elsewhere, another tab or
   view, closing the tab or the window — writes the title typed, as other
   fields do, and a save or a run writes it first; a refused title goes
   back to the one before and holds the save or the run back
   (the owner, 2026-11-24, on #574; amended in #624).
   `title_is_committed_when_the_field_is_left`,
   `title_edit_is_committed_by_a_save_and_a_run`,
   `title_that_is_refused_goes_back`.
8. A title with a quote, a backslash or a non-ASCII letter is written so
   that the analysis reads the same title back. `title_edit_escapes`.
9. On a read-only or observed board, and while the document's code has
   errors, the double click opens nothing. `title_edit_needs_an_editable_board`.
10. While the field is open the board's keys (Delete, Escape on a
    selection, the move keys of DIAG-06b) do not act: Escape leaves the
    field and keeps the selection. `title_edit_keeps_the_boards_keys`.
11. Split is unchanged: its header has no name. `split_header_unchanged`.
12. Design review at 960 × 540 and 1600 × 1000 against the board, the
    edited state included; a review view `review_ws05_header` is added.

## Implementer's captures the owner will judge

Not on the approved board; chosen as the least surprising, shown in the
pull request's QA section:

- The field's width: the title's width, at least the header's least
  name width, at most the room the title has. It grows with its text over
  the kind and the file name, Main and Cleanup staying in place, or to the
  room of a component's row (#624); a longer text scrolls in the field.
- The header of a component (its name, file name, *Component*).
- The pointer over the title is the text cursor, the only sign that it
  can be edited; no pencil.

## Not in scope

*Files* (WS-05b); the tab's label, which stays the file name; Run
results' header and the exported name (REP-05 #363); a way to find a
test by typing its title (the owner, 2026-11-24: no ticket).

## Files touched

`folder/diagram.rs` (`Header`, `HeaderFit`, the edit's state), the
existing path a model edit takes to the text (`session.apply_model_edit`
through `board_edit.rs`), `folder/header_tests.rs`,
`folder/acceptance_tests/ws05a.rs`, `testing.rs` (the review view).
Crate: `apiforge-desktop` only; if writing the title needs a change in
`apiforge-script` or `apiforge-project`, stop and escalate.

## Regression risk

- The header's fit (KTLO-12, KTLO-23): `header_tests.rs` and criterion 2.
- The board's keys while a field has the keyboard (LAY-23, DIAG-06b):
  criterion 10.
- The text beyond the first line after the edit (REQ-13 #540): the
  text comparison of criterion 6.
