# LAY-42: Changing a field's source is reached from the field and applied only on Apply

- Size: M · Owner review: 25 min · Risk: `visible UI`
- State: **ready** — mock-up accepted by the owner on 2026-11-18. Decided in principle on
  2026-11-18 (#296): Q5 = A, a change of source is staged — nothing is
  written until *Apply*, with a summary above it, worded without `@after`;
  Q6 = A, *Change source* on the relation statement a chip or line click
  keeps opens the Inspector at that field. Ready · issue #439 · LAY sprint, part of
  its close, beside the cards track
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

A tester who has traced `second.body.roomId` to `rooms.body.items[1].id`
wants to make that one field read something else, without typing code and
without wondering what else moved. Today the editor is three screens down
the Inspector in a folded section, and every click in it is written at
once. The way there starts at the relation just traced, and the window
says what the change is — and what it is not — before it is made.

## What the window does today

`main` 2f6922c, `double-booking`, 1280 × 800:

- The editor exists: `second` → *Configuration* → scroll to the bottom →
  open the folded **Body value source** → `roomId`: *Result of another
  step*, Step, Output, Response field, with sample values. Only steps that
  run earlier are offered.
- Choosing Step `first` is written at once as `roomId: first.body` (the
  whole body), before a field is chosen; the document is unsaved from
  that click. Nothing says what changed or that only this field did.
- The statement a click on a chip or a line keeps
  (`rooms.body.items[1].id → second.body.roomId`) leads nowhere.

## What the owner will see

- New: **Change source** at the end of a kept statement. A click selects
  the reading step and opens the Inspector's *Configuration* at that
  field: the section that holds it open (*Body value source* for a body
  field), the field's editor at the top of the panel.
- Changed: in a field's *Result of another step* editor, choosing a Step,
  an Output or a Response field **writes nothing**. The editor is framed
  and headed `roomId · new source · not applied yet`; the controls show
  the choice; the board, the code and *Save* stay as they were.
- New: under the controls, the summary, then **Apply** and **Cancel**:

  > `second` · Body · `roomId` will read `first.body.roomId` (sample
  > `"room-a"`) instead of `rooms.body.items[1].id` (sample `"room-b"`).
  > Only this field changes.
  > `second` still runs 8th, in `booked → Yes`, after `first`.
  > `rooms` is still read by `first` and `calendar`.

  Each sentence is computed, never fixed text:
  1. the field, the new and the old reference as the code writes them,
     each with its sample (the last run's value when there is one, else
     the sample response; `no sample` when neither has the field);
  2. always;
  3. the step's number in the run, the branch it stands in (left out
     when it stands in none), and `after <new source>`;
  4. who still reads the old source — or "`rooms` is no longer read by
     anything", or, when the step still reads it through another field,
     "`second` still reads `rooms` in `start`".

  One more sentence when it applies: "This value does not exist when
  `booked → No` runs" (the new source stands in another branch than the
  reader).
- *Apply* writes the one reference — one undo step, the document unsaved
  — and the editor returns to its plain state. *Cancel*, Esc, selecting
  another step or closing the Inspector drop the choice; nothing was
  written, so nothing is asked.
- The wording never says `@after`, "ordering wire" or "edge": position is
  the step's number and its branch, which stay true when the order is the
  written order (LANG-13).
- Unchanged: which steps are offered (only earlier ones); the other kinds
  of source (a literal, a template, an operator: written as typed, as
  today); the *Body* text box; drag and drop of a variable onto a field;
  the stored samples.
- Removed or hidden, migrated, default behaviour: nothing.

## Mock-up

Throwaway branch `mock/lay-close` (`review_laymock_source`), the room
project's `double-booking` at 56 %, 1280 × 800, in
[`docs/qa/2026-11-18/LAY-42/`](../qa/2026-11-18/LAY-42/):

| View | Capture |
| --- | --- |
| The room chip of `second` clicked: the traced line, both ends ringed, and the kept statement `rooms.body.items[1].id → second.body.roomId` ending with *Change source* | [lay42-change-source-on-statement](../qa/2026-11-18/LAY-42/lay42-change-source-on-statement.png) |
| After *Change source* and the choice of `first` · `roomId`: the Inspector at the field, framed `roomId · new source · not applied yet`, Step `first`, Response field `roomId`, the summary, *Apply* and *Cancel*. The board still shows `rooms` as the source and *Save* is off: nothing is written | [lay42-staged-source](../qa/2026-11-18/LAY-42/lay42-staged-source.png) |

The summary in the capture is computed by the mock from the document (the
samples, the 8th place, the branch, the two readers), not typed in. The
button on the statement is painted only.

## Questions for the owner — decided

The owner accepted the mock-up on 2026-11-18 with every recommended option (1 = A, 2 = A, 3 = A; quiz, recorded on #439). The questions are kept below as asked. Folded into the ticket by Opus 5.5: Fable has no tokens left until 2026-11-22 (model policy).

Merge order: after LAY-15 (#442: `canvas/edge.rs`, `trace.rs`, `layout.rs` `Card::header_rect`, `app.rs`) and LAY-39 (#438: `canvas/readers.rs`) where this ticket touches the same files; merge `origin/main` first. Beside the cards track; the Inspector part is its own.


1. **The two views as drawn.** **A (recommended):** approve. **B:** send
   back with what to change.
2. **The board while a choice is held.** **A (recommended):** unchanged
   until *Apply* (as drawn): what is on the board is what is in the file.
   **B:** the new source is shown as a dashed line beside the old one — a
   preview, and one more kind of line to tell apart.
3. **Leaving with a choice held** (another step selected, the Inspector
   closed, Esc). **A (recommended):** the choice is dropped, as *Cancel*:
   nothing was written. **B:** the window asks *Apply or discard?* first.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay42.rs` on
a temporary copy of `fixtures/scenarios/lay/rooms/`, `double-booking`,
1280 × 800; the summary's sentences as unit tests beside the function that
builds them.

1. A click on `second`'s room chip keeps the statement with a button
   named *Change source of second.body.roomId*; a statement that is only
   pointed at has none.
2. A click on it selects `second`, opens *Configuration* with *Body value
   source* open and the `roomId` editor inside the panel's view; the kept
   trace stays; the document is not unsaved.
3. Choosing Step `first` and then Response field `roomId` leaves the
   document's text, its unsaved mark, the board's wires and the undo
   history exactly as they were; the editor shows `first` / `roomId` and
   the heading *not applied yet*.
4. The summary reads, sentence by sentence, as in "What the owner will
   see" for this edit — `"room-a"`, `"room-b"`, `8th`, `booked → Yes`,
   `after first`, `first and calendar`.
5. The other forms, on edited copies: the old source left with no reader
   → "is no longer read by anything"; the reader keeps another read of it
   → "`second` still reads `rooms` in …"; a step outside any branch → no
   "in …"; no sample for the field → `no sample`; a source in the other
   branch → the "does not exist when … runs" sentence.
6. *Apply* changes exactly one expression: the `.afs` differs from the
   original in the one line of `second`'s body (`rooms.body.items[1].id`
   → `first.body.roomId`), one undo step restores it, and the step's
   number, its branch and every other binding are as before (LAY-34's
   rule).
7. *Cancel*, Esc, selecting `first`, and closing the Inspector each leave
   the document as in criterion 3 and the editor, reopened, on
   `rooms` / `items[1].id`.
8. No text of the summary contains `@after`, "wire" or "edge" (asserted
   over criteria 4 and 5).
9. The summary, *Apply* and *Cancel* are inside the Inspector at
   1280 × 800 and at 960 × 540 without a horizontal scroll; the statement
   with its button stays whole inside the board.
10. Editing a literal, a template part or an operator's operand writes as
    on `main` (the existing Inspector tests run unchanged).

## Regression potential

- The Inspector writes a node on every change (`Action::UpdateNode`); a
  held choice must not leak into it, nor into the field drafts restored
  after a restart (`inspector/drafts.rs`): criteria 3 and 7 assert the
  document, and the drafts' tests run unchanged.
- The statement's box is laid whole inside the board (LAY-14, `lay14`
  criterion on the box): it grows by the button; `lay14` runs unchanged
  and criterion 9 covers the new width.

## Not in scope

- The edge chips, the jump and *Back* (LAY-15). A changes view in the
  window (the language and editor sprint; `git diff` is the accepted way
  for 0.2.0, Q9 = A, #296). Staging for other kinds of source or for
  inputs edited as text. Opening *Body value source* by default.

## Technical appendix

- `inspector/binding.rs` `reference_editor` (the mock wraps it: the same
  controls on a held copy); `inspector/mod.rs` (the section opened, the
  scroll to the field — `Focus` scrolls to an input today, the field path
  is new); `canvas/trace.rs` and `readers::paint_statement` (the button);
  a command from the board to the Inspector carrying the reader, the input
  and the field path of the kept wire.
- The summary reads `apiforge_core::graph::sequence` (the number),
  the ordering edge into the step for its branch, the bindings for the
  readers, and `inspector/response.rs` for the samples. All of them exist
  under the written-order language too; "only earlier steps are offered"
  is the same rule there.
