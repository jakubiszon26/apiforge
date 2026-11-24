# LANG-25: A request's argument is read as `arg.name`

- Size: M, at its upper edge · Owner review: 20 min · Risk: format, default
- State: Analysis. The shape is the owner's decision 2 on #589
  (2026-11-24, [LANG-23](LANG-23.md), F3 option A); the readings under *To
  confirm* are the planning model's. Not labelled: the analysis awaits the
  owner's approval and, for the `format` flag, a second model's read
  ([autonomy](../process/autonomy.md#what-makes-a-ticket-ready)).
- Issue: #592
- Analysis model: Fable · Implementation model: Opus

## Goal

A request declares `argument cart` and reads it as `arg.cart`. Today it is
read as `input.cart`, while `input productId` of a component is read as
`param.productId`: one word means two things.

## What the owner will see

- New: `arg.name` in a request's own URL, headers, query and body.
- Changed: the window writes `arg.name` when it writes a request, so a
  request card shows `{{arg.cart}}` where it showed `{{input.cart}}`; the
  completion offers `arg`.
- Changed: `input.name` in a request still runs, with a warning whose fix
  writes `arg.name`; `apiforge fix --folder` rewrites a whole project.
- Removed or hidden: nothing. Migrated: nothing automatically.
- Default behaviour that changes: none in a run; the language version
  stays 2. `apiforge fix --folder` on a project in the current language
  no longer only says "nothing to convert": it applies the fixes of this
  ticket (criterion 3).

## Mock-up

Not applicable: no control changes; a card shows the text of its source.

## Acceptance criteria

1. In a request, `arg.name` reads `argument name` and lowers to the same
   stored document as `input.name` does today. Test: S
   `lang25_criterion_1_*` (the two spellings lower equal).
2. `input.name` in a request stays valid and runs; it carries the warning
   *A request's argument is read as `arg.name`* with a fix that rewrites
   that reference alone. Test: S `lang25_criterion_2_*`.
3. `apiforge fix --folder` on a project **already in language 2** applies
   this fix to every working source instead of stopping at "nothing to
   convert". Today it cannot: `plan_fix` returns `Planned::Current` for
   such a project (`apiforge-project/src/language.rs:205-207, 228-230`) and
   the command prints that sentence (`apiforge-cli/src/fix.rs:53-59`). The
   pass writes through the guarded commit LANG-20 defines (its read set
   and refusal, not restated here), leaves the manifest and the published
   revisions untouched, changes no other byte, and changes nothing when
   run again; `--check` reports what it would write and writes nothing. A
   language-1 project is converted first, as today, and the pass follows
   in the same run. Test: C `lang25_criterion_3_*`.
4. The printer writes `arg.name` (`print.rs:303`); an edit on the board
   rewrites only the statements it changes. Test: S round trip, D one
   request edited on the board.
5. A file with a statement whose handle is `arg` means what it means
   today: `arg.x` reads that statement everywhere, argument reads stay
   `input.name` in that file, the fix leaves them and the warning at the
   handle says to rename it. Test: S `lang25_criterion_5_*`.
6. Completion inside a request offers `arg` and its argument names and no
   longer offers `input`; find and rename treat `arg` as they treat
   `input` (`complete.rs`, `far.rs:123`). The messages *Unknown request
   argument* and *Expected a request argument name* are given for `arg.`
   too. Test: S.
7. The reference writes `arg.name` in *Names*, in *Requests* and in every
   example, says in one sentence that `param.` is what a file receives
   from outside (a scenario's parameter, a component's input), and that
   0.2.0 and 0.2.1 refuse `arg.name` at its line. The first line stays
   *Language version: 2*. Test: the reference test.

## To confirm at acceptance

- **Criterion 5: the handle wins.** `arg` is not a reserved word, so
  `arg = …` is valid today. Reserving it would make a valid file invalid
  and raise the language version (LANG-00, section 2, rule 1), which
  decision 2 does not ask for.
- **Criterion 2: a warning, not silence.** Without a diagnostic the fix
  has nothing to hang on; every existing project gets warnings until its
  one `apiforge fix`.

- **Criterion 3: the command widens** from "convert a project to the
  build's language" to "and apply the fixes the current language
  offers". That is the owner's decision 2 read literally ("`apiforge fix`
  rewrites it"), and a change of what a command does by default, so it is
  his to confirm. The alternative keeps the command as it is and leaves
  the rewrite to the fix on the warning (editor, `check_text`), one
  reference at a time: smaller, no `default` flag, and a project keeps
  its warnings until someone clicks through them.
- **Size.** If the second read finds criterion 3 takes the ticket over
  M, it is split: 25a the language (criteria 1, 2, 4 to 7), 25b the pass
  of the command.

## Not in scope

Reading a component's inputs as `input.name` (option B, not chosen);
refusing `input.name` (it would raise the version; a later language step
may); published revisions.

## Technical appendix

`lower.rs:1273` (`Binding::Argument`), `print.rs:303`, `complete.rs:653-654,
894`, `far.rs:123`, the MCP server serves the reference file as built.
Regression: a file with `input.name` lowers and runs exactly as before
(criteria 1 and 2); the existing request tests stay unchanged. To check
first: whether any label on the board or in the inspector says "input" for
a request's argument; if one does, that is a visible change and returns to
the owner.
