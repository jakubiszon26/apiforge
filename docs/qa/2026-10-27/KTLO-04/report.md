# KTLO-04: A registered or imported document seals its identities at its first successful analysis — QA report

- Branch: `feat/ktlo-04-seal-on-analysis`; acceptance tests `9115822`, fix
  `c3ddbf9`, clippy correction of one test line `155d789` (same assertion),
  `main` merged in `fd9ca90`.
- Mock-up: not applicable, no interface change. Ticket: KTLO-04
  (`docs/tickets/KTLO-04.md`, arriving with #73). Owner decision of
  2026-10-26 on #6: seal the block identities and the layout at the first
  successful analysis.
- Built with: `cargo test --locked` (debug). No release build and no captures:
  nothing on screen changes. What changes is one Git-tracked file,
  `.apiforge/diagram.json`.
- Machine: Linux, 3 cores, shared throughout with other sessions building the
  workspace (load average 11–17), so timings are upper bounds.

## What changed, in one place

The record is sealed where an analysis result is applied
(`ProjectSession::apply_analysis`), when all of these hold: the result is
valid, the session may write the project, the document's text is its saved
file (not dirty, no external conflict), and `diagram.json` has no record under
the document's key. `initialise` analyses through the project load, `import`
and `register_existing` through `analyse_now`; all three apply their result
there, so this one call covers them, and also an invalid source whose saved
text later turns valid. `create_document`'s own call to `seal_new_checkpoint`
became redundant and was removed. `seal_new_checkpoint` keeps its early returns:
unchanged bytes write nothing, and a write that fails is left to the next save
(ADR 0010).

## Scenarios

All in `crates/apiforge-project/tests/acceptance.rs`. A document's blocks are
compared as (identity, position) pairs, read in the session and read again by a
fresh load of the saved project.

| # | Criterion | Test | At `9115822` (before the fix) | At `c3ddbf9` and `fd9ca90` |
| --- | --- | --- | --- | --- |
| 1 | `initialise` and `import` keep identities; record present | `a_registered_or_imported_document_keeps_its_block_identities_when_the_project_is_reopened` (no `#[ignore]`) | fails: "initialisation analysed the source cleanly, so its record is sealed" | passes |
| 2 | `register_existing` the same | `a_source_registered_in_an_open_project_keeps_its_block_identities_when_it_is_reopened` | fails: "the registration analysed the source cleanly, so its record is sealed" | passes |
| 3 | invalid seals nothing; first valid analysis seals | `an_invalid_source_seals_its_identities_at_its_first_successful_analysis` | fails: "the first analysis that turned the saved source valid seals its record" | passes |
| 4 | the accepted limitation | `a_document_closed_before_its_first_successful_analysis_gets_fresh_identities_until_one_seals_them` | fails at its last part: "the first successful analysis in a writing session seals the record" (the drift part already held) | passes |
| 5 | migration records | `a_migrated_project_has_a_record_for_every_document_and_revision` | passes | passes |
| 6 | a failing write does not fail the registration | `a_checkpoint_that_cannot_be_written_does_not_fail_the_registration_or_import` | passes (no write attempted yet) | passes |
| 7 | verification commands | see *Verification* | — | pass, one load-sensitive desktop test aside |

### 1. Initialise and import (criterion 1)

1. Write a valid two-block scenario `found.afs`; `ProjectSession::initialise`
   with it registered.
2. Expected: `.apiforge/diagram.json` holds a record keyed by the document's ID
   before the session is closed; a fresh load shows the same blocks.
   Observed: as expected.
3. Open the project, import `incoming.afs` (valid). Expected and observed: its
   record is written by the import; after closing, a fresh load shows the
   imported blocks unchanged, and `found.afs`'s blocks are still the original
   ones.

### 2. Registered in an open project (criterion 2)

`ProjectSession::create`, write `found.afs`, `register_existing`. Observed: the
record is present when `register_existing` returns; a fresh load shows the same
blocks.

### 3. Invalid, then valid (criterion 3)

1. Initialise with an invalid `found.afs`. Observed: analysis `Invalid`, no
   record (`diagram.json` not written at all).
2. Write the valid source to the file on disk while the session is open, as
   another editor or a `git pull` would; run the external check. Observed: the
   document is reloaded, analysed valid and its record written; a fresh load
   after closing shows the blocks the session had.
3. Import an invalid copy; no record. Edit it valid **in the session** and
   settle. Observed: valid, but still no record — the text is unsaved, and
   shared checkpoints describe saved valid source (ADR 0010). Save. Observed:
   `SaveOutcome::checkpoint` is true, the record is present, and a fresh load
   shows the saved blocks.

### 4. The stated limitation (criterion 4)

1. Initialise with an invalid `found.afs`, close. Fix the file on disk while
   the project is closed.
2. Load the project twice read-only (as a second window or `apiforge check`
   does). Observed: the two loads give **different** block identities, and no
   record is written — the drift the owner accepted with the decision.
3. Open the project with a writing session. Observed: the first successful
   analysis seals the record; every later load shows the same blocks.

### 5. Migration (criterion 5)

`migrate_workspace` of `fixtures/workspace.json`. Observed: in every migrated
project, every registered document has a record under its working key, every
published revision has its revision record, and every scenario's blocks equal
those of a fresh load. This held before the fix as well: migration writes the
folder with `prepare_project`, which captures every record in its own
transaction, not through the registration path. The existing migration suite
(13 tests) passes unchanged.

### 6. A write that fails (criterion 6)

1. `ProjectSession::create`; then, outside the session, write a valid but
   different `.apiforge/diagram.json` (an empty record list).
2. `register_existing` a valid `found.afs` and `import` a valid copy.
   Expected: both succeed, both documents valid and not dirty, and the changed
   `diagram.json` is not overwritten. Observed: as expected; a later load lists
   both documents as valid.

That the test really reaches the failed write was checked by mutation: with the
early return after a failed commit in `seal_new_checkpoint` replaced by a
panic, this test — and only this test — fails.

### What a Git-tracked `diagram.json` looks like

Before a registration in a newly created, empty project: the file does not
exist. After `register_existing` of the valid two-block scenario above, the
file appears with one record (abridged; the second block and the empty
`cleanup` graph are cut):

```json
{
  "formatVersion": 1,
  "records": [
    {
      "key": {
        "kind": "working",
        "documentId": "dd5797dc-913d-44d1-9be2-a8b2e8f06d02"
      },
      "sourceHash": "09b0a921da10c5f999028278d30dc46b758fdc23b7115b4766d1ae2b9ec775ef",
      "contextDigest": "a5c5669c5ff03184e1db42a18c15d9dd8cbc3579d1c207a133b5dd3ed7f824cd",
      "modelVersion": 2,
      "cacheVersion": 1,
      "semanticDigest": "e09ad61c868fbcd780803721ceb9d12ce5d1cfa32b5c405f0f73742beb2040f9",
      "identityBase": {
        "kind": "scenario",
        "document": {
          "id": "5ad51ea7-52fa-4652-b56b-bd703580470a",
          "name": "Main",
          "graph": {
            "nodes": [
              {
                "id": "node-eae5fe58",
                "handle": "answer",
                "kind": "value",
                "position": { "x": 0.0, "y": 0.0 },
                "autoPlaced": true,
                …
```

The whole file is 75 lines for two blocks. Its format is unchanged; the
project's `.gitattributes` already lists it with LF line endings. In a Git
working copy the owner sees it as a new file after the first registration of a
valid source, and as one more record per registered or imported document after
that.

## Verification

- `cargo fmt --all -- --check`: clean (at `c3ddbf9` and at `fd9ca90`).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: at
  `c3ddbf9` it failed on one line of the criterion 3 test
  (`clippy::cloned_ref_to_slice_refs`); corrected in `155d789` without changing
  the assertion. At `fd9ca90`: `Finished`, no warning.
- `cargo test --workspace --all-targets --locked`:
  - at `c3ddbf9`, before merging `main`: **1 035 passed, 0 failed, 23 ignored**
    over 52 test binaries, exit status 0, desktop suite included.
  - at `fd9ca90`, after merging `main`, with `--no-fail-fast`: **1 043 passed,
    1 failed, 23 ignored**. The failure is
    `folder::tests::a_large_source_follows_the_caret_once_its_edit_is_analysed`
    ("storage did not become idle", its 15 s wait), the same test the KTLO-19
    pull request recorded failing under load. It does not reach the new code:
    its project is created by `create_document`, which already had its record,
    and its edit is unsaved, so nothing is sealed. Run alone three times on the
    same tree, it passed three of three (15.0 s, 18.5 s, 18.2 s, load average
    12–13), and it passed in the branch's CI run on both platforms (see *Windows*).
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit
  status 0.
- The new acceptance tests, ten runs in a row of `--test acceptance` at
  `fd9ca90`: **10 of 10** passed (12 tests each, 3.2–4.4 s).
- Migration suite (`--test migration`): 13 passed. Desktop suite (it opens
  projects through the same session API): 384 passed, 21 ignored, and the one
  load-sensitive failure above.

## Windows

CI run [40502031607](https://github.com/jakubiszon26/apiforge/actions/runs/40502031607)
on `fd9ca90`: **success** on both `Build & Test (windows-2022)` and
`Build & Test (ubuntu-24.04)`, the whole workspace suite included, so the
load-sensitive desktop test above passed there too. The acceptance tests are
platform-independent (no permissions, no `#[cfg]`). No local Windows run was
made; the owner's own Windows build is still to come.

## Not verified

- A project whose `diagram.json` is unreadable or of an unsupported version:
  the session keeps it untouched and seals nothing, as before
  (`checkpoints_unavailable`); not covered by a new test.
- A desktop session observed writing the record in the window: no interface
  test was added, and none of the ticket's criteria asks for one. The desktop
  opens and registers through the same session calls the acceptance tests use.
- Many documents without a record opened at once: each is sealed by its own
  small write during the load; the cost was not measured.
- The budget and fuzzer commands of the development guide: not run; nothing
  they cover changed.
- Regression smoke list: not run. Nothing visible changes; item 7 (save and
  reopen keeps layout and identities) is what the acceptance tests check at
  library level.

## Regression smoke list

Not run for this ticket; see *Not verified*.
