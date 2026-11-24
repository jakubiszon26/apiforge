# WS-05b: Files lists scenarios and components by title

- Size: M · Owner review: 15 min · Risk: `visible UI`
- State: **Approved, ready for implementation** (issue #596) · decided by
  the owner on 2026-11-24 on the board, recorded on #574: **B** · split
  from WS-05 on 2026-11-24; after [WS-05a](WS-05a.md) (#574) · labelled
  `ready:opus` · REQ milestone
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

A tester who knows a test by its title finds it in *Files*.

## Finding

Looked at on `main` (`1923fcd3`). A row is `folder/explorer.rs`,
`explorer_row`: the file name (`Row::name`) as text, as the accessible
name and in the name of its menu; the path on hover. The rows are in
`tree_order`, by path. The dialogs are `folder/file_prompts.rs` (*Rename
x.afs*, *Move x.afs*, *Delete x.afs?*). The session keeps a document's
last valid model while its code has errors, so a title outlives a typo.

## What the owner will see

- Changed: a registered scenario's or component's row reads its title,
  then its file name in grey where the row has room.
- Removed or hidden: at the list's default 240 px the file name of most
  rows is cut or pushed out; it stays on hover, in the tab, in the
  header and in every dialog. Decided with B on the board, which shows
  it.

## Mock-up

Approved as a whole on 2026-11-24: `docs/qa/2026-11-24/WS-05/title.png`,
column B. From `review_ws05_title` on `mock/ws-05-req-15` at `aa6518b6`.

## What a row reads

| Row | Text | Hover |
| --- | --- | --- |
| A registered scenario or component with a model | The title; then the file name, grey, smaller | The title, then the path |
| The same with no model yet, or an empty title | The file name, as today | The path, as today |
| The same while its code has errors | The last valid title; the error mark as today | The title, then the path |
| A missing document | The file name, grey, as today | As today |
| An unregistered `.afs`, any other file, a folder, the project | As today | As today |

Settled here, each the least surprising of what the board leaves open:

- **Two tests with one title** read alike; the grey file name tells
  them apart where it fits, the hover always. No number is added and no
  problem is reported: the language allows it.
- **A long title** ends in an ellipsis at the row's edge; the file name
  takes only what the title leaves and is left out below 40 px, never
  cut to a stub of two letters.
- **Order**: unchanged, folders first and then by file name, as on disk
  and in an external editor. A row does not move when its title is
  edited.
- **Marks** (unsaved, error, conflict), the selected and focused states,
  drag and drop, and the keys of the list: unchanged. A dragged row
  moves the file.
- **The row's menu and the dialogs act on the file and say so**: the
  menu and *Rename*, *Move*, *Delete* keep the file name in their
  heading, and under it, for a scenario or component with a title, a
  grey line with the title. *Rename* renames the file; the title is
  edited in the header (WS-05a).
- **The accessible name** of a titled row is the title, then the file
  name (`Double booking is refused, double-booking.afs`), so a screen
  reader hears what is shown and a test still finds the row by its file.
- **The tab** keeps the file name; its hover becomes the title, then the
  path, as the row's.

## Acceptance criteria

Tests in `folder/acceptance_tests/ws05b.rs`, on WS-05a's fixture plus a
component, an unregistered `.afs` and a second scenario with the same
title.

1. Each row reads what the table says, for all five kinds.
   `rows_read_titles`.
2. The hover of a titled row and of its tab is the title, then the path;
   other rows as today. `hover_names_title_and_path`.
3. At 240 px a long title ends in an ellipsis inside the row and clear
   of the mark at its right edge; the file name is whole, cut to at
   least 40 px, or absent. `long_title_at_the_default_width`.
4. Widening the list to 480 px shows title and file name whole for the
   long fixture. `wide_list_shows_both`.
5. Two scenarios with one title: both rows are listed, each opens its
   own file, and their accessible names differ. `one_title_twice`.
6. The order of the rows equals today's for the fixture, and does not
   change when a title is edited. `order_is_by_file`.
7. A title edited in Code or in the header shows in the row once the
   analysis has run, saved or not; code with errors keeps the last valid
   title and shows the error mark. `row_follows_the_title`.
8. *Rename*, *Move* and *Delete* of a titled row keep the file name in
   the heading and add the title's line; renaming the file leaves the
   title and the `scenario` line as they were. `dialogs_name_the_file`.
9. A row dragged to a folder moves the file; the unsaved, error and
   conflict marks stand where they do today. `drag_and_marks_unchanged`.
10. The tests that find a row by its file name pass through one helper,
    `files_row(harness, file)`, which matches the end of the accessible
    name; no test finds a row by a position. `explorer.rs`' own tests and
    `tab_band_tests.rs` stay green.
11. Design review at 960 × 540 and 1600 × 1000 against the board; a
    review view `review_ws05_files` is added.

## Implementer's captures the owner will judge

Not on the approved board; shown in the pull request's QA section:

- A component's row by its name (the decision says scenarios; a list
  with one kind by title and the other by file would read as a defect).
- Two rows with one title at 240 px and at 480 px.
- The *Rename* and *Delete* dialogs with the title's line.
- The row of a scenario whose code has errors.

## Not in scope

The header and the edit (WS-05a); sorting or grouping by title; a filter
or search of *Files* (the owner, 2026-11-24: no ticket); the Library,
which lists components by name already; Trash and the *Move* dialog's
folder list.

## Files touched

`folder/explorer.rs` (`Row`, `explorer_row`, `explorer_rows`),
`folder/file_prompts.rs` (the three headings), `folder/chrome.rs`
(`tab_face`, the hover), the tests named above,
`folder/acceptance_tests/ws05b.rs`, `testing.rs` (the review view).
Crate: `apiforge-desktop` only.

## Regression risk

- Tests and assistive navigation that find a row by its name:
  criterion 10.
- The list's row arithmetic (rows are exactly 24 px; one line per row
  stays): criterion 3 and `explorer.rs`' tests.
- File operations taking a title for a name: criterion 8.
