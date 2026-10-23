# Integration review — 2026-10-19

Reviewed the integrated desktop, runner and editing changes through `627aa84`.
This is a targeted integration review, not release acceptance for Linux and Windows.

## Findings corrected

- A shared pure argument could activate disconnected HTTP consumers. The scheduler
  now loads argument dependencies without treating them as new control roots.
  A real HTTP regression test verifies that only the connected request is sent.
  See [the reachability decision](../decisions/0003-start-reachability.md).
- Cleanup duration was included when checking the main phase deadline. Each phase
  now keeps its own budget.
- Closing the desktop abandoned active cleanup. The close handler now cancels the
  main phase and polls responsively for cleanup, with a 12-second upper bound.
- Duplicating cleanup blocks lost references to main-graph outputs. Duplication now
  uses the same external-reference handling as paste; undo/redo is covered.
- Background import/export completion did not wake an idle window. The worker now
  requests a repaint on exit, including failure.
- The report could lose its expanded height after collapse, and selecting a nested
  component step could jump to its parent details. Both have regression tests.

## UI corrections

The native window was checked against the React layout. The sidebar is 235 points
wide; the header reserves a title column and aligns run actions on the right. The
library has a taller search field, compact rows and an example menu. The board uses
a compact selection menu instead of a row of disabled actions. Persistent board
controls appear immediately rather than fading in during the first frames.

Card labels retain readable minimum font sizes while zoomed out. Comparison cards
show the predicate and omit the unused expected operand for unary predicates. The
report fills the requested height, has a visible resize grip, remembers its expanded
size and leaves space for the board in smaller windows.

## Verification

On Linux: 195 workspace tests passed, including the compatibility corpus and local
HTTP regressions; clippy with warnings denied, formatting, the fixture generator's
`--check` and the release build passed. Actual native viewport captures were reviewed
with and without the comparison inspector. Captures are generated under
`test-results/apiforge-native-{shop,inspector}.png` and are not source artifacts.

Windows execution, accessibility and installer/release acceptance remain unverified
locally. Passing these checks does not make the migration production ready.

## Follow-up: workflow, navigation and durable saving

The next increment adds a searchable **Find step…** dialog (Ctrl+K) with arrow-key
selection, Enter to open configuration and camera reveal, and Escape to dismiss.
Input-event integration tests exercise navigation, duplicate/undo and text paste with
an existing block clipboard, without mutating the document during navigation.

Three real loopback HTTP tests cover prices → cart → items → cleanup with dynamic
response values, the false IF branch, redirects without retry, and cancellation of
an in-flight request followed by cleanup exactly once.

Autosave now holds at most one active and one waiting workspace. Generation-aware
status cannot acknowledge a newer edit when an older write completes. Backups use
atomic replacement; Unix directory-sync errors are reported, including for newly
created parent directories. Saving validates the import byte limit before changing
files. Unsupported and oversized saved documents cannot be silently replaced by a
backup or autosave. A failed startup load opens a temporary session with autosave
disabled; a failed close-time save keeps the window open with a retry action.

The full workspace now passes **215 tests**, clippy with warnings denied, formatting,
fixture verification and a release build on Linux. The updated release binary was
also validated with `--check` and a native inspector capture. Native CI smoke/capture
probes are configured for both platforms; see [their exact scope](native-ci.md).
Hosted CI and Windows execution remain unverified, and Windows directory-metadata
durability under power loss remains a platform limitation.
