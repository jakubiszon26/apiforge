# KTLO-03: Migration refuses a real six-project workspace

- Size: M · Owner review: 30 min · Risk: `migration`, `visible UI` (the stop
  message)
- State: Approved 2026-10-27 (issue #5) — decisions 1 and 2 taken 2026-10-26,
  3 and 4 taken and the mock-up (5) approved 2026-10-27; after KTLO-04 (#6)
- Mock-up branch: `mock/ktlo-03-migration-stop` (throwaway; captures in
  `docs/qa/2026-10-27/KTLO-03/`, posted on #5)
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

Upgrading from the JSON workspace migrates the owner's real workspace, or
says precisely why it cannot and stops. Today it fails on one of six projects
with `Verification { reason: "a scenario differs after reloading" }`, activates
no catalogue, prints the reason to standard error and silently opens the
legacy window (`docs/stage-ii-review-notes.md`, *A real workspace that
migration refuses*; `main.rs:168-181`).

## What the owner will see

- New: when migration is refused, a small window *Migration stopped* with the
  project's name, the reason ("a scenario differs after reloading", and which
  scenario), what was left untouched (the originals and the staged folders)
  and two buttons: *Quit* and *Open the previous workspace* — the latter does
  what `--legacy` does. Nothing is activated.
- Changed: the legacy window no longer opens by itself after a refused
  migration (decision 1). The other two reasons startup opens the workspace
  window today are untouched (decision 4): `activation::decide`
  (`folder/activation.rs:61-104`) sends five other causes there — another
  window holds the lock, a storage error opening the workspace, a backup
  decision, unreadable drafts, a load error — and only the refused migration
  changes route.
- Removed or hidden: the silent fallback.
- Migrated or converted: the owner's workspace, once the cause is fixed —
  expected to be KTLO-04's missing checkpoint: a scenario whose identities
  were never sealed gets fresh ones on reload, which `verify`
  (`migration.rs:689-714`) rightly rejects.
- Default behaviour that changes: a refused migration ends at the stop
  window, not in the legacy window.

## Mock-up

Captured on `mock/ktlo-03-migration-stop` (`aed3a17`) and posted on #5:
`mock-migration-stop-960x540.png`, `mock-migration-stop-1600x1000.png` and the
close-up `mock-migration-stop-window.png`. The window is 480 × 276 points
(24-point margins around a 432-point text column; a long reason wraps to five
lines), centred on the screen; a 20-point bold title, one paragraph with the
project's name, the reason and the scenario, a muted "left untouched, nothing
activated" line, and two standard buttons, *Open the previous workspace* and
*Quit*, neither styled as the primary action. It is the first window the
person sees, before any project window exists, in the application's theme.

## Decisions for the owner

1. **(taken 2026-10-26) A refused migration reports and stops**; `--legacy`
   only on explicit request.
2. **(taken 2026-10-26) The acceptance fixture is synthetic**: a six-project
   JSON workspace built in the repository that reproduces the refusal; if it
   does not, the ticket returns to the owner for an anonymised copy.
3. **(taken 2026-10-27: A) How the stop is shown.** A: the *Migration
   stopped* window above, with *Quit* and *Open the previous workspace*.
   B (declined): print to standard error and exit with a non-zero code —
   invisible to a person who starts the application from an icon.
4. **(taken 2026-10-27: A) The other legacy-window causes.** A: untouched —
   only a refused migration ends in the stop window; the lock, the backup
   decision and the three error cases keep opening the workspace window as
   today. B (declined): the stop window for the error cases too. C
   (declined): for all six, which would need the lock and the backup decision
   handled in a new window.
5. **(taken 2026-10-27) The mock-up** as a whole.

## Acceptance criteria

1. A synthetic six-project workspace (`crates/apiforge-project/tests/fixtures/`
   or built in the test) reproduces `Verification { reason: "a scenario
   differs after reloading" }` on `main` before any fix, with KTLO-04's change
   absent; the test states which project and why. If it cannot be made to
   reproduce, the ticket stops here and goes back to the owner (decision 2).
2. With KTLO-04 merged, `migrate_workspace` on the fixture succeeds: six
   folders, every scenario equal after reload, the catalogue activated.
3. A migration that is still refused (a second fixture with a deliberately
   broken project) returns a `MigrationError` naming the project and the
   scenario; the originals and the staged folders are byte-identical before
   and after; nothing is activated.
4. `managed_start` reports the refusal as its own `Start` variant (not
   `Legacy`) and `main` opens the stop window of decision 3 instead of the
   workspace window; `--legacy` still opens it.
5. The stop window shows the reason and the two buttons with accessible
   names; *Open the previous workspace* opens the legacy window; *Quit* exits
   (interaction tests at both screen sizes; the accessibility audit passes).
6. The verification commands of the development guide pass; the migration
   tests of `apiforge-project` pass unchanged.

## Not in scope

The legacy mode's removal (after KTLO-06's parity list); migrating a
workspace another window holds; the content of the reason strings beyond
naming the project and the scenario.

## Technical appendix

- `verify` (`migration.rs:689`) compares `document.model()` with the legacy
  `Scenario`; a fresh-identity reload differs in node ids, so the fixture must
  be a project whose scenario entered without a sealed checkpoint — build it
  by preparing a project through the legacy `WorkspaceStore` writer, not the
  session, so no `diagram.json` record exists.
- `MigrationError::Verification` carries only `project_id` and a static
  `reason` (`migration.rs:56-60`), and `verify` (`migration.rs:688-714`) never
  records which scenario differed. The window needs the project's *name* and
  the scenario's name: extend the variant (or the reason) with both; the
  `Display` text follows. This is part of criterion 3.
- `activation::decide` (`folder/activation.rs:61-104`) chooses
  `Start::Legacy(Some(..))` for six causes; add `Start::Refused(..)` and route
  only the migration error there. `managed_opening` (`main.rs:167-180`) builds
  `Opening::Workspace` for `Legacy`; the new variant builds a small eframe
  window (one mode, fixed 480 × 276 points, a `CentralPanel` with 24-point
  margins — egui's default root margin differs, so set it) that holds the
  message and the two buttons.
- The fixture generator should live beside the migration tests and be
  reusable for KTLO-06's removal work.
- Size: the stop window and its tests are the bulk; the fixture ~100 lines;
  M. If the stop window pushes it over M, the fixture and the `Start`
  variant go first (criteria 1–4) and the window second (criterion 5).
