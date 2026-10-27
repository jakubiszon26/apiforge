# KTLO-04: Identity checkpoint at initialise, register_existing and import

- Size: S–M · Owner review: 30 min · Risk: `format`
- State: Approved 2026-10-26 (issue #6) — seal at the first successful analysis;
  analysis written 2026-10-27
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
- Migrated or converted: nothing retroactively. A document registered before
  this change and never since analysed gets its record at its next successful
  analysis, which is the same rule.
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
3. An invalid source registered or imported seals nothing; when its text is
   edited into a valid document and the analysis applies, the record appears
   then (a test that registers invalid text, edits it to valid, settles, reopens
   and compares ids).
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

- `ProjectSession::create` (`session.rs:434-436`) is the pattern:
  `analyse_now(index)` then `seal_new_checkpoint(index)`. `import`
  (`register.rs:328`) and `register_existing` (`register.rs:398`) call
  `analyse_now` and stop; `initialise`/`initialise_with` (`register.rs:125-134`)
  go through the same path. Adding `seal_new_checkpoint` after each
  `analyse_now` covers every valid source at once.
- "First successful analysis" for a source that was invalid at registration:
  where the analysis result is applied and `seal_valid(index)` is called
  (`session.rs:638`), also call `seal_new_checkpoint(index)` when the shared
  file has no record under this document's key (`DiagramFile.records` lacks
  `DocumentKey::Working { document_id }`). `seal_new_checkpoint` already
  returns early when the bytes are unchanged, so an existing record is never
  rewritten by this path; saves keep their own route.
- `seal_valid` (`execution.rs:118`) seals the in-memory valid snapshot, not
  the file; do not confuse the two.
- Migration: `migration.rs` builds the folder through the session API, so the
  change in the session covers it; criterion 5 pins that.
- Size: the change is a handful of lines; the tests and this ticket are the
  bulk — expect S.
