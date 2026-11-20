# DIAG-06g: Undo and Redo restore the file byte for byte

- Size: M · Owner review: 15 min · Risk: `default` (a comment typed in
  Code becomes a step of Undo)
- State: **Decided; waits for the independent review before it is
  labelled.** Split from DIAG-06e on 2026-11-20. After LANG-13 (#386)
  only; before DIAG-06e (#494).
- Issue [#502](https://github.com/jakubiszon26/apiforge/issues/502) · LANG
  sprint, first half, a condition of 0.2.0 · branch
  `feat/diag-06g-undo-restores-the-file`
- Analysis model: Fable 5.2 · Implementation model: Opus 5.5

## Goal

What Undo brings back is the text that was there — its free comments,
its blank lines, the author's attribute order — not a text written again
from the board.

## What happens today

Read from `main`, `apiforge-project/src/session.rs`: the history of a
document keeps earlier **documents**. Undo installs the earlier document
and writes the text by editing the current text towards it
(`edit_in_place`), or by printing it whole. What a deleted statement
held that the document does not know — a free comment inside an `if`'s
braces, blank lines, the order the author gave its attributes — is not
restored. The same history also takes the document of each settled edit
made in Code, and the completion of a save may replace history entries
while newer edits arrive (`storage_work.rs`).

## What the owner will see

- Changed: Undo after an edit on the board gives the file back byte for
  byte; Redo gives the edited file back byte for byte.
- Changed: every settled edit of the text is a step of the history,
  also one that changes no block — a comment, a blank line. Today a Code
  edit enters the history only when it changes the document
  (`apply_analysis`), so a comment typed after a board edit is no step,
  and an Undo that restored the earlier text would silently take it
  away. After this ticket Undo takes back the **last** change, whatever
  it was: after a delete on the board and then a comment typed in Code,
  the first Undo takes back the comment, the second the delete. Nothing
  is skipped, nothing is merged, and nothing typed is lost without being
  the thing undone. (For the owner's veto at acceptance: it is one more
  kind of step in an order that is otherwise today's.)
- Removed or hidden, migrated: nothing.

## Mock-up

Not applicable.

## Acceptance criteria

Tests in `crates/apiforge-project/tests/diag06g.rs` (P) and
`crates/apiforge-desktop/src/folder/acceptance_tests/diag06g.rs` (D), on
`shop-30` and `rooms/booking-day`, with a free comment, blank lines and
attributes in a non-canonical order placed in the statements the tests
edit.

1. For each edit the board can make on `main` — adding a block, a step
   from *Next*, wiring a value, deleting a step with its checks, deleting
   an `if`, an Inspector change, a paste — one Undo makes the file
   byte-identical to before the edit and one Redo byte-identical to after
   it. Tests: P `diag06g_criterion_1_*`, one per edit; D for the delete
   of an `if`.
2. A chain of three edits undone three times and redone three times
   passes through the same texts, byte for byte.
   Test: P `diag06g_criterion_2`.
3. **Interleaved with Code.** The history takes an entry when a settled
   analysis finds the **text** changed, whether or not the document did.
   A delete on the board, then a comment typed in Code and settled: the
   first Undo gives the text after the delete (the comment gone, because
   it is what was undone), the second the text before the delete; two
   Redos give both back, the comment last. A Code edit that is not yet
   settled when Undo is pressed is settled first, so it is a step and is
   not lost. No Undo gives a text that never existed.
   Tests: P `diag06g_criterion_3_*`: the comment; a blank line; an edit
   still being typed.
4. **During a save.** An edit made while a save is in progress is kept;
   after the save completes, Undo and Redo still pass through the same
   texts, among them the one that was being saved.
   Test: P `diag06g_criterion_4_*` (the save held by the test).
5. Where the history is cleared today — a reload, a foreign write taken
   in, a conflict resolved — it is cleared; no kept text is installed
   over a file it no longer follows. Test: P `diag06g_criterion_5_*`.
6. The memory a history takes is bounded as today (the number of entries
   and the source limit of 1 MiB per entry). Test: P `diag06g_criterion_6`.
7. The document after an Undo is the one read from the restored text,
   with the identities and positions it had (KTLO-26).
   Test: P `diag06g_criterion_7`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| Undo and Redo everywhere (the window, an agent's session) | criteria 1–4; the session's history tests unchanged in what they assert |
| Identities and layout after Undo (KTLO-26, checkpoints) | criterion 7; `round_trip` and `recovery` suites unchanged |
| The editor's own Undo while typing in Code (ADR 0007) | not touched: this is the document's history; `editor_tests` unchanged |

## Not in scope

What a delete, a move or a paste does (DIAG-06e, DIAG-06a, DIAG-06d,
DIAG-06f): each asserts this ticket's property for its own edits.

## Technical appendix

`apiforge-project/src/session.rs` (a history entry keeps the source text
with the document; `apply_analysis` pushes an entry when the text
changed, not only when the document did; Undo and Redo install both), `storage_work.rs` (the
entries a save's completion rewrites keep their texts).
