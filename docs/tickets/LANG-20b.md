# LANG-20b: `apiforge fix` converts a project in guarded commits: the plan, its read set, and the finish

- Size: M · Owner review: 20 min · Risk: `format` (sources rewritten, the
  record and the manifest written)
- State: **Decided; waits for the independent review before it is
  labelled.** Split from LANG-20 on 2026-11-20. The owner's decision of
  that day (#481, question 4 = B): file by file, with a record. After
  LANG-20a (#481), which gives the record and the project's states.
- Issue [#497](https://github.com/jakubiszon26/apiforge/issues/497) · LANG
  sprint, first half, a condition of 0.2.0 · branch
  `feat/lang-20b-fix-command`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

One command brings a project written for 0.1.0 to language 2 as far as
it can each time it is run, never under a plan that no longer fits the
files, and finishes the project when nothing is left — also when what is
left is only to say so in the manifest.

## What the owner will see

- New: `apiforge fix --folder <directory>` converts every source it can,
  lists per file what the person does by hand, and when every source is
  in language 2 and passes the check raises the manifest and removes the
  record. Exit code 0 only when the project is in language 2 afterwards.
- New: `--check` prints the same report and never writes; `--as-written
  <relative.afs>` (repeatable) lists a source the person converted by
  hand, when it passes the language-2 check.
- Migrated or converted: only by the person's command. Published
  revisions are not touched. Removed: nothing.

## Mock-up

Not applicable: a command and its text output.

## The plan

One type, made by one function and used by the command, by LANG-20c's
MCP tool and by LANG-21a's window. Making it writes nothing.

1. **What it read** — the read set: the fingerprint of the manifest, of
   the record (or that there is none) and of **every** source, also those
   it will not write. For the window, a source with unsaved changes is
   read from its buffer and marked so.
2. **Each source is exactly one of:**
   - *done*: listed, and passes the language-2 check;
   - *done, with errors*: listed, and does not pass — named with its
     errors; it is not converted again;
   - *to convert*: not listed; its 0.1.0 reading and the order written
     agree, and the converted text passes the language-2 check — also a
     source without any `@after`, which is then only listed;
   - *blocked*: not listed; a step runs nowhere today, or runs in another
     place than it is written, or the converted text would not pass the
     check — named with the statement, the reason and what to do;
   - *unreadable*: missing, not text, too large, or it does not parse —
     named; never counted as done or as nothing left.
3. **What it would write**: the converted texts, the record after them,
   and the manifest when it changes.
4. **Whether it finishes**: after this plan every source is *done* — then
   the manifest is raised to language 2 and the record removed, **also
   when there is nothing to convert** (every source was listed and the
   last error has just been repaired, or the last blocked source was
   deleted).

## The commit

5. A plan is committed only against what it read. The project store
   gains what it lacks today: a commit takes the plan's **read set** and
   refuses, writing nothing, when any fingerprint in it — of a file it
   writes or not, the manifest and the record among them — is not the
   one on disk when the commit takes the store's lock
   (`apiforge-storage/src/project/transaction.rs`: today only the files
   written are compared, and the manifest against its state when the
   commit began). ADR 0011 is amended.
6. The sources of a plan are written in commits that each fit the
   store's limits (64 MiB, 1 024 operations). Every commit carries the
   record for the sources it converts, so each leaves a project being
   updated in a consistent state; the last one finishes. A single source
   over the limit is *unreadable*.
7. An interrupted commit is rolled forward by the store's recovery at
   the next writable open, as any commit (ADR 0011); a foreign write in
   between stops recovery with its conflict, as today. After recovery
   has run, no source is converted and unlisted.
8. **After a save.** A caller that saved something between the plan and
   the commit (the window's *Save all and update*) does not carry the
   old plan over: it makes the plan again from the disk and commits the
   new one only when it would write exactly what the shown one would —
   the same sources with the same texts, the same blocked and unreadable
   lists, the same finish. Otherwise nothing is converted and the new
   plan is what the person is shown. The manifest written is built from
   the manifest read by the new plan, so settings saved in between are
   kept. This function is part of this ticket; the window only calls it.

## Acceptance criteria

Tests in `crates/apiforge-cli/tests/fix.rs` (C),
`crates/apiforge-project/tests/lang20b.rs` (P) and
`crates/apiforge-storage/tests/acceptance.rs` (T), on the language-1
copy of `rooms` of LANG-20a and its variant with one statement moved
above the step it follows and one request nothing orders.

1. All sources convertible: one run converts them, raises the manifest
   to language 2 and leaves no record; exit code 0; `check` passes and
   `run` runs. Test: C `lang20b_criterion_1_*`.
2. One source blocked: the others are converted and listed, the blocked
   one is byte-identical, the project is being updated (LANG-20a), the
   exit code is 1, the report names file, line, statement, reason and
   what to do. Test: C `lang20b_criterion_2_*`.
3. A second run after the person's edit: the done sources are not read
   by their 0.1.0 reading and not rewritten (bytes equal), the edited one
   is converted, the project is finished. Test: C `lang20b_criterion_3_*`.
4. A language-1 source without any `@after` whose request ran nowhere in
   0.1.0 is blocked and names the request on every run while unlisted;
   with `--as-written` it is listed, when it passes the language-2
   check, and refused with its errors when it does not.
   Test: C `lang20b_criterion_4_*`.
5. **The finish without a conversion.** Every source listed, one with an
   error: the run converts nothing, reports that source and its errors,
   exit code 1. After the error is repaired — or the last blocked source
   is deleted from the project — the next run writes only the manifest
   and removes the record; exit code 0. Test: C `lang20b_criterion_5_*`.
6. Each kind of source in the plan: a missing file, a file that does not
   parse, a conversion whose result fails the check, a listed source
   with an error — each is reported under its own heading, none is
   converted, none is counted as done. Test: P `lang20b_criterion_6_*`.
7. **A stale plan writes nothing.** Between the plan and the commit: a
   *done* source is changed on disk; a source the plan does not write is
   changed; the manifest is changed although the plan does not write it;
   the record is changed. In each case the commit refuses, no file
   changes, and the run says which file. Tests: T
   `lang20b_criterion_7_read_set_*`, P `lang20b_criterion_7_*`.
8. More sources than one commit holds (the test lowers the limits): the
   run converts them in several commits, the project is consistent after
   each, and finished after the last; a failure in the second leaves the
   first's sources converted and listed. Test: P `lang20b_criterion_8_*`.
9. An interrupted commit: after the next writable open every converted
   source is listed. Test: P `lang20b_criterion_9`.
10. *After a save* (rule 8): with a dirty buffer saved in between and
    nothing else changed, the new plan equals the shown one and is
    committed; with a foreign write during the save, with a setting saved
    in between (kept in the manifest written), and with a buffer whose
    saving changes what is blocked — the first two as said, the last
    converts nothing and returns the new plan.
    Test: P `lang20b_criterion_10_*`.
11. `--check` prints the report and writes nothing; its exit code is the
    run's. A project already in language 2: nothing changes, exit code 0.
    No comment is lost in any converted source.
    Tests: C `lang20b_criterion_11_*`.

## Regression potential

| What can break | Covered by |
| --- | --- |
| Every other commit of the store (saves, publication, trash): the read set is optional and empty for them | the storage suites unchanged; criterion 7's T tests |
| A converted source judged again by its 0.1.0 reading | criterion 3 |
| An old source taken for a converted one | criteria 4 and 6 |
| Settings saved in the window lost by the update | criterion 10 |
| The command writes a person's files | criteria 2, 3, 11 |

## Not in scope

The record's format and what a build does with it (LANG-20a). The MCP
tool (LANG-20c). The window (LANG-21a, LANG-21b). What the fix of one
file does (LANG-13).

## Technical appendix

`apiforge-storage/src/project/transaction.rs` (the read set of a
commit), ADR 0011; `apiforge-project/src/language.rs` (the plan, the
commit, *after a save*); `apiforge-cli` (`main.rs`, a new `fix.rs`);
`README.md`, `docs/folder-projects.md`. This ticket does not touch
`apiforge-script`.
