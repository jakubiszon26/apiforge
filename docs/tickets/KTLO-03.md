# KTLO-03: Migration refuses a real six-project workspace

- Size: M · Owner review: 30 min · Risk: `migration`, `visible UI` (the stop
  message)
- State: Analysis — decisions 1 and 2 taken 2026-10-26 (issue #5); decision 3
  (how the stop is shown) and its mock-up pending; after KTLO-04 (#6)
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
  window today — another window owns it, a backup decision is needed
  (`main.rs:162-166`) — are untouched (decision 4, recommended).
- Removed or hidden: the silent fallback.
- Migrated or converted: the owner's workspace, once the cause is fixed —
  expected to be KTLO-04's missing checkpoint: a scenario whose identities
  were never sealed gets fresh ones on reload, which `verify`
  (`migration.rs:689-714`) rightly rejects.
- Default behaviour that changes: a refused migration ends at the stop
  window, not in the legacy window.

## Mock-up

To be captured on `mock/ktlo-03-migration-stop` and posted on #5: the stop
window at its natural size on a 960 × 540 and a 1600 × 1000 screen, with a
long reason text. It is the first window the person sees, before any project
window exists, so it is a plain native window in the application's theme.

## Decisions for the owner

1. **(taken 2026-10-26) A refused migration reports and stops**; `--legacy`
   only on explicit request.
2. **(taken 2026-10-26) The acceptance fixture is synthetic**: a six-project
   JSON workspace built in the repository that reproduces the refusal; if it
   does not, the ticket returns to the owner for an anonymised copy.
3. **How the stop is shown.** A (recommended): the *Migration stopped* window
   above, with *Quit* and *Open the previous workspace*. B: print to standard
   error and exit with a non-zero code — invisible to a person who starts the
   application from an icon.
4. **The other two legacy-window reasons** (another window owns the
   workspace; a backup decision is needed). A (recommended): untouched — they
   are resolved in the workspace window, which is why it opens. B: the stop
   window for them too, with the same two buttons.

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
- `managed_start` returns `Start::Legacy(Option<reason>)`; add
  `Start::Refused(MigrationError)` and keep `Legacy` for the two untouched
  reasons. `main.rs:168-181` builds `Opening::Workspace` for `Legacy`; the new
  variant builds a small eframe window (as the legacy window is built, one
  mode, fixed size) that holds the message and the two buttons.
- The fixture generator should live beside the migration tests and be
  reusable for KTLO-06's removal work.
- Size: the stop window and its tests are the bulk; the fixture ~100 lines;
  M. If the stop window pushes it over M, the fixture and the `Start`
  variant go first (criteria 1–4) and the window second (criterion 5).
