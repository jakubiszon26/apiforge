# LAY-39: A chip keeps the index of the element it reads

- Size: S · Owner review: 10 min · Risk: `visible UI`
- State: decided by the owner on 2026-11-18 (Q7 = A, #296): a chip keeps
  the last index and field, `← rooms…[1].id`, and is never a bare `←` while
  there is room; text only. Accepted on a capture before merge (#296,
  Plan A) · issue #435 · LAY sprint, part of its close
- Analysis model: Fable · Implementation model: Opus

## Goal

Two bookings read two different rooms, `rooms.body.items[0].id` and
`rooms.body.items[1].id`, and their chips say the same thing: `← rooms.id`,
`← rooms`, or a bare `←`. The index — the only difference, and in the
owner's acceptance task the defect itself — is not on the board, and a
person has to guess which of a card's chips is the room. The chip keeps the
index, so two reads of two elements look different at every zoom where a
chip has text at all.

## What the board does today

Looked at on `main` 2f6922c, `double-booking` and `booking-day` of the room
project (`fixtures/scenarios/lay/rooms/`), 1280 × 800, after Tidy up:

- A chip's text is the source step and the **last field** of the path:
  `rooms.body.items[1].id` → `← rooms.id`; `first` reads `items[0].id` and
  says `← rooms.id` too (76 %).
- Where the gap has no room for it, the chip says the source step alone,
  `← rooms`, and then the arrow alone. At 54 % `second` has `← token`, `←`
  and `← slot`: `← rooms` is a few pixels wider than the gap allows, so the
  one chip the task is about is the unnamed one.
- Pointing at a chip states the whole reference
  (`rooms.body.items[1].id → second.body.roomId`); that stays as it is.

## What the owner will see

- Changed: a chip whose read goes through a list element keeps the **last
  index** of the path. Its text, widest first, each tried by the rule that
  decides today whether a text fits the gap:
  1. `← rooms…[1].id` — step, `…` for what is left out, the last index and
     the field after it;
  2. `← rooms…[1]` — step and index;
  3. `← …[1]` — the index alone;
  4. `←` — only where not even 3 fits.

  `…` is written only where something between the step and the index is
  left out; a path that ends at the element (`rooms.body.items[1]`) reads
  `← rooms…[1]` from the start. With two or more indices the last one is
  kept (`a.body.rows[2].cells[0].value` → `← a…[0].value`).
- At the sizes the task is read at this gives: about 95 % and above
  `← rooms…[1].id`; at 76 % `← rooms…[1]`; at 54 % `← …[1]`. `first` shows
  `[0]` and `second` `[1]` at each of them.
- Unchanged: a chip whose path has no index (`← login.token`, then
  `← login`, then `←`); a variable's chip (`← cartId`, then its badge
  number, LAY-25); the statement shown on pointing and on a click, and the
  chip's accessible name; the counts at the source (`→ 3`); where a chip
  stands, its size rule, the gap Tidy up leaves, every route. No size rule
  changes: the text is chosen among more forms, by the same width test.
- Removed or hidden, migrated, default behaviour: nothing.

## Mock-up

Not drawn: the owner decided the text itself (Q7 = A, #296). The pull
request carries two captures for acceptance before merge — `double-booking`
at 54 % and `booking-day` at 76 %, 1280 × 800, with `first` and `second`
in view.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay39.rs` on
a temporary copy of `fixtures/scenarios/lay/rooms/`, tidied, chips on,
checks folded, 1280 × 800; criteria 1–3 also as unit tests of the text in
`canvas/readers.rs`.

1. The chip of `second.body.roomId` offers `rooms…[1].id`, `rooms…[1]`,
   `…[1]` in that order; `first`'s offers the same with `[0]`; `calendar`'s
   read of `rooms.body.items[0].id` in its path offers the `[0]` forms.
2. A path with no index gives today's forms and nothing else: `slot.start`
   → `slot.start`, `slot`; a variable's chip gives its name and its badge
   number. A path ending at an element gives `x…[n]`, then `…[n]`; a path
   with two indices keeps the last.
3. The statement and the accessible name of every chip of the three room
   scenarios are byte-for-byte what `main` gives.
4. On `double-booking` at 54 % (zoom set after Fit) no chip of `first`,
   `second` or `calendar` whose read has an index is drawn as a bare `←`;
   the room chips of `first` and `second` are drawn with different texts,
   one containing `[0]`, the other `[1]`.
5. On `booking-day` at 76 % the room chips of `first` and `second` are
   drawn as `← …[0]` and `← …[1]` (restated by the owner's decision A on
   #435: the criteria follow the room there is).
6. At 100 % on `double-booking` the two chips read `← rooms…[0]` and
   `← rooms…[1]` (restated the same way).
7. Nothing else moves: on the three room scenarios at 54 %, 76 % and
   100 % every chip and count keeps the room LAY-37 asks for (from each
   other, from the cards, from the lines at rest — `lay37`'s assertions run
   on these boards), and a chip with an index ends at the same stub as
   before (its right edge and row are those of a chip with the old text;
   only its width follows its text). The change touches the choice of text
   only: no constant of size, gap or reach changes in the diff.
8. The chip tests of LAY-23, LAY-25, LAY-32 and LAY-37 pass unchanged. If
   one of their boards has a read through an index, the set of texts that
   test allows gains the forms above and nothing is removed from it; the
   pull request names the test and the chip.

## Regression potential

A wider chip in a gap (form 1 is wider than `← rooms.id`) could meet a line
or another chip. The placement of LAY-37 already tries each form in turn
and falls to the next where one has no clear place, so the risk is a chip
that shortens earlier than its neighbour, not an overlap; `lay37`'s
assertions (chips clear of each other, of cards and of lines) run on the
room scenarios in criterion 7 and cover it.

## Not in scope

- The chip's shape and arrow (LAY-38 #427, LAY-2).
- The body row of a compact card (`Body · 3 fields`), the field list at a
  count (LAY-18).
- Lines through another card's chips (W5 of the trial, #296: LAY-2 unless
  the owner misreads it).
- A wider gap, a smaller font, a new truncation of a step's name: no size
  rule changes here.

## Technical appendix

- `canvas/readers.rs`: `texts()` builds `text` and `short` from the
  reference's `path`; `lay_tags()` passes `[text, short, "←"]` to
  `fitting()`, which keeps each form that fits `room` and always the last.
  The change is the list of forms: for a path with an index, the three
  above instead of `text` and `short`. Whether `Tag` grows a list of forms
  or `text`/`short` change meaning is the implementer's choice; `lay31`
  compares `tag.text == "login.status"` (no index, unchanged).
- The index is the last `[n]` in the reference's `path` string; the field
  is what follows it after a `.`, if anything.
- Why the bare arrow at 54 %: `room = GAP · zoom − stub − 2` is about
  44 px, `← rooms` with its padding is a little over, `← token` a little
  under. `← …[1]` is about 30 px.
- The fixture `fixtures/scenarios/lay/rooms/` is added by this ticket's
  first commit (the same files ride on LAY-15's branch; identical content
  merges cleanly).
