# KTLO-08: Windows and Linux QA captures from CI

- Size: M · Owner review: 10 min · Risk: none
- State: Awaiting owner acceptance (branch `feat/ktlo-08-qa-captures`)
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal
QA reports must be able to cite Windows and Linux screenshots of the folder
window in each of its three modes (Code, Diagram, Split) at the two standard
sizes (960 × 540 and 1600 × 1000), taken from the release binary built by CI
for the branch under review.

## What the owner will see
- New: nothing in the application. Every CI run whose capture phase
  completes uploads, per platform, an artifact `qa-captures-<os>` with six
  captures of the folder window (a failed run may upload fewer; a Windows
  capture of another size than asked is a warning on the run, not a failure);
  QA reports gain a Windows section built from them.
- Changed: nothing. `--screenshot` gains two optional developer flags,
  `--mode` and `--size`; without them it captures exactly what it did before.
- Removed or hidden: nothing.
- Migrated or converted: nothing.
- Default behaviour that changes: none.

## Mock-up
Not applicable: no interface change.

## Acceptance criteria
1. `--screenshot OUT.png --folder PATH --mode code|diagram|split --size WxH`
   is accepted in any order; each option is optional and its absence keeps
   today's behaviour (the folder's own mode, a 1600 × 1000 window).
   Test: `screenshot_takes_a_folder_mode_and_a_window_size` (`main.rs`).
2. `--mode` without `--folder` (with `--project`, or the demo workspace) is
   rejected with an error naming `--mode` and `--folder`; `--size` is accepted
   with `--project`. An unknown mode, a malformed size and a size below the
   960 × 540 minimum are rejected with an error naming the option.
   Test: `screenshot_rejects_a_mode_without_a_folder_and_a_malformed_size`.
3. The folder window opened with a requested mode shows that mode once the
   project is open, at both standard sizes, and does not write it back as the
   project's view. Test:
   `a_mode_requested_at_startup_is_shown_and_not_written_back`
   (`folder/acceptance_tests.rs`).
4. On both CI runners the demo shop project, as migrated by the managed-startup
   step, is captured in all six combinations into
   `test-results/qa/<mode>-<W>x<H>.png`, each verified non-empty within a
   45-second probe, and uploaded as `qa-captures-<os>` even when a later step
   fails. QA step: open the artifacts of a workflow run of the branch.

## Not in scope
- Pixel comparison of the captures; they are evidence for a person.
- Captures of dialogs, menus or a selected block; a QA report that needs them
  takes them by hand or through a later ticket.
- Changing the existing `native-window-<os>` artifact.

## Technical appendix
- `crates/apiforge-desktop/src/main.rs`: parsing of `--mode` and `--size`;
  the window's inner size comes from `--size`.
- `crates/apiforge-desktop/src/folder/mod.rs`: `FolderApp::request_mode`
  applies the mode when the session is installed; `capture_view` writes the
  folder's own mode while the requested one is shown, so a capture never
  changes what the project restores next time.
- `.github/workflows/rust.yml`: a fifth phase of both folder-project steps
  captures `projects/shop` from the migration phase.
- The capture's pixel size is the logical size times the display scale; the
  CI displays use a scale of 1.

Actual: 471 changed lines (M; estimated S) · owner review and rounds of
changes recorded at acceptance.
