# LANG-20f: The update after a save: the plan is made again and committed only when nothing else has changed

- Size: S · Owner review: 10 min · Risk: `format` (it writes what
  LANG-20b writes)
- State: **Decided; labelled with the ruling of the independent review.**
  Split from LANG-20b on 2026-11-20. After LANG-20b (#497); LANG-21a
  (#482) calls it.
- Issue [#501](https://github.com/jakubiszon26/apiforge/issues/501) · LANG
  sprint, first half, a condition of 0.2.0 · branch
  `feat/lang-20f-update-after-a-save`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

When the person was shown a plan made from unsaved text and then saves
and updates in one step, what is converted is what they were shown, or
nothing.

## What the owner will see

Nothing new on screen by this ticket: it is the function behind
LANG-21a's *Save all and update*.

## Mock-up

Not applicable.

## The rule

The caller has a **shown plan**, made from buffers (LANG-20b marks which
sources were read from a buffer), and the list of what its save wrote:
each path with the fingerprint the save acknowledged, the manifest among
them when settings were saved.

1. A **new plan** is made from the disk.
2. Its read set must equal the shown plan's, where for each path the
   save wrote the expected fingerprint is the one the save acknowledged,
   and for every other path the one the shown plan read. Any other
   difference is a foreign write: nothing is converted and the new plan
   is returned to be shown, with the path.
3. The new plan must do what the shown one did: the same sources to
   convert with the same resulting texts, the same blocked, unreadable
   and with-errors lists, the same finish. Otherwise nothing is converted
   and the new plan is returned.
4. Then the new plan is committed as LANG-20b commits any plan. The
   manifest it writes is built from the manifest it read, so settings
   the save wrote are kept.

## What a save reports

The session's save (`ProjectSession::save`, and the save of all) returns
today that it succeeded. This ticket makes it return, for each path it
wrote — sources, text files, the manifest when settings were saved — the
fingerprint it wrote, and for each path that failed, that it failed. The
window's *Save all* hands that on (LANG-21a changes
`folder/saving.rs`); nothing else about saving changes.

## Acceptance criteria

Tests in `crates/apiforge-project/tests/lang20f.rs`, on LANG-20a's
language-1 `rooms` and its blocked variant.

1. One buffer saved, nothing else changed: the update converts what the
   shown plan listed. Test: `lang20f_criterion_1`.
2. A foreign write during the save to a source the plan would convert,
   to a *done* source that stays valid, and to a source the plan does not
   touch: each time nothing is converted and the path is named. (The
   second is the case where the outputs would be equal and only the read
   set tells.) Test: `lang20f_criterion_2_*`.
3. Settings saved in the same step are in the manifest after the update;
   a manifest changed by anyone else is a foreign write.
   Test: `lang20f_criterion_3_*`.
4. A saved buffer that changes what is blocked, or adds an error to a
   listed source: nothing is converted, the new plan is returned.
   Test: `lang20f_criterion_4_*`.
5. The save's report: one buffer, several, settings alone, and a save
   where one path fails — each path is reported once with what was
   written or that it failed. Test: `lang20f_criterion_5_*`.
6. A save that reported a failure for any path: the function is not
   called by the window (LANG-21a); called so, it refuses.
   Test: `lang20f_criterion_6`.

## Regression potential

None beyond LANG-20b's: it adds one caller-side check before LANG-20b's
commit.

## Not in scope

Saving (the window's *Save all*, LANG-21a). The plan and the commit
(LANG-20b).

## Technical appendix

`apiforge-project/src/language.rs`, `session.rs` (what a save returns).
