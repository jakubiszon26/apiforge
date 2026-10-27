# KTLO-04: Identity checkpoint at initialise, register_existing and import

- Size: S–M · Owner review: 30 min · Risk: `format`
- State: Awaiting acceptance in PR #74 (issue #6) — seal at the first
  successful analysis, decided 2026-10-26; analysis 2026-10-27; implemented
  the same day, actual 593 changed lines (M, inside the S–M estimate), 1 round
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

A document that entered a project through `initialise`, `register_existing`
or `import` keeps its block identities and layout across reopening, exactly
like a document the project created. Today only `create` seals a checkpoint;
the other three register the source and analyse it, but write no record, so
every later load allocates fresh identities for their blocks (the `#[ignore]`d
`a_registered_or_imported_document_keeps_its_block_identities_when_the_project_is_reopened`
in `crates/apiforge-project/tests/acceptance.rs:59` says so). KTLO-03 (#5)
suspects the same gap behind the refused migration.

## What the owner will see

- New: nothing on screen.
- Changed: after a file is registered or imported, or a folder initialised,
  `.apiforge/diagram.json` (Git-tracked) gains a record for each such document
  as soon as it analyses cleanly — immediately for a valid source, at the
  first later analysis that turns valid for an invalid one. Reopening the
  project then shows the same node identities and positions.
- Removed or hidden: nothing.
- Migrated or converted: nothing by a separate step. A document that has no
  record yet — registered before this change, or fixed on disk while the
  project was closed — gets its record at its next successful analysis in a
  session that can write, which is the next writable open of the project
  (the window, `apiforge apply`, a live CLI run): one small commit to
  `diagram.json` per such document during the load. Read-only sessions
  (`apiforge check`, a second window) write nothing.
- Default behaviour that changes: a registration or import of a valid source
  now writes one more Git-tracked file. That is the owner's decision of
  2026-10-26 (`format`).

## Mock-up

Not applicable: no interface change.

## Owner decision, taken 2026-10-26 (issue #6)

**Seal the block identities and the layout at the first successful
analysis**, not at registration and not left to settle at the first save:
the identities come from a correct parse, the checkpoint never seals garbage,
and the Git-tracked file appears only for sources that really analyse.
Accepted with it: a document reopened before its first successful analysis
can still drift; the acceptance test states this limitation.

## Acceptance criteria

1. `a_registered_or_imported_document_keeps_its_block_identities_when_the_project_is_reopened`
   runs without `#[ignore]` and passes: after `initialise` with a registration
   and after `import`, `.apiforge/diagram.json` holds a record for the document
   and the node ids read before and after reopening are equal.
2. The same for `register_existing` (a found `.afs` registered in an open
   session) — one more case in the same test or its own test.
3. An invalid source registered or imported seals nothing. A source fixed
   *on disk* (another editor, a pull) is sealed by the analysis that finds it
   valid; a source fixed *in the session* is sealed by its save, not by its
   analysis — ADR 0010: shared checkpoints describe saved valid source, so an
   unsaved edit never seals (a test for each half; the implementation guards
   on: the session can write, the document is not dirty, no external
   conflict, no record under its key yet).
4. A document registered, closed before its analysis finished and reopened
   gets fresh ids — the stated limitation — asserted as such, so the test names
   what the decision accepted.
5. Migration (`apiforge-project/src/migration.rs`) prepares every scenario and
   component with a record, as it analyses them; the existing migration tests
   pass unchanged and a test asserts the record is present after `migrate_workspace`
   for a registered-style source.
6. A checkpoint write that fails (read-only project, lock held) does not fail
   the registration or import that already succeeded (ADR 0010): the existing
   rule in `seal_new_checkpoint` stays and is covered by a test.
7. The verification commands of the development guide pass.

## Not in scope

The migration refusal itself (KTLO-03, #5, which follows this ticket); any
change to the record format; sealing on save.

## Technical appendix

- One seam is enough: where an analysis result is applied and
  `seal_valid(index)` is called (`apply_analysis`, `session.rs:638`), a guarded
  `seal_first_checkpoint(index)` seals when the session can write, the
  document is clean and unconflicted, and the shared file has no record under
  its key. `initialise` analyses inside `load_saved`, and `import` and
  `register_existing` apply their `analyse_now` results through the same
  path, so separate calls after `analyse_now` (as this appendix first said)
  would do nothing; `create_document`'s own `seal_new_checkpoint` call became
  redundant and went. `seal_new_checkpoint` returns early on unchanged bytes,
  so an existing record is never rewritten by this path.
- `seal_valid` (`execution.rs:118`) seals the in-memory valid snapshot, not
  the file; do not confuse the two.
- Migration: `prepare_project` writes every record directly, not through the
  session; criterion 5 held before the change and pins that it keeps holding.
- Not measured: the cost of a writable open that seals many record-less
  documents at once (one commit each). Noted for the backlog if it shows.
- Size: 49 lines of code (`session.rs` 43, `register.rs` 6), 336 of tests,
  208 of QA report.
