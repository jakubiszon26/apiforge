# Format v2 / M1 acceptance

Verified locally on Linux, 23 October 2026. Native Windows execution remains a
release gate. The next feature milestone is M4 (scenario language and CLI).

## Revision editing correction

The acceptance test first failed because `initialize_outputs` read the original
component definition instead of the graph currently being edited. New, renamed
and changed Export blocks could disappear on saving a revision. Recreating all
Exports also changed their IDs and lost their notes, layout and ordering links.

Revision output rows now come from the selected Export blocks on the current
board. Materialization reuses existing Export identities and preserves graph
order, notes, positions and `autoPlaced`. Deleted or disabled outputs are removed
with their ordering edges. A literal Export does not automatically turn into a
new public argument; explicitly exposing its value still works. Changing the
source in the builder takes priority over the earlier binding. Existing instances
keep their revision. An Export's next-step menu uses ordering, never an invented
data output.

## Automated evidence

- The converted project/run fixtures and TypeScript expression compatibility
  check pass. Converted simulation runs retain their recorded reports with the
  reviewed ADR 0004 divergences.
- Native v2 integration tests cover nested component Exports, instance isolation,
  active null versus skipped/missing outputs and earlier outputs surviving a
  later failed Export.
- Core editing tests cover current-board additions/renames, identity/layout/note
  preservation, branch ordering, deletion/disable and direct public arguments.
- A desktop egui test adds a new Export, edits its value, opens the revision
  dialog and saves through Ctrl+Enter, verifying the new revision and the
  unchanged old definition.
- The combined M1/S1 workspace has 281 passing tests. One ignored helper is
  executed by the process-lock test. Formatting, Clippy `-D warnings`, both
  fixture checks and the workspace debug build pass.

## Native GUI evidence

The KWin test used a private virtual session with isolated HOME and copied fixture
data. It opened the `Aktywna promocja` component, added an Export from the library,
configured an explicit null value, renamed it to `qa_export` and saved revision 2.
Inspection of the saved workspace confirmed both outputs, the retained original
node IDs and the unchanged revision 1. No Live API or host workspace was used.

Local evidence under `.gui-qa/runs/component-export-revisions/`:

- `actions.json` — native input and session lifecycle;
- `component-board.png`, `added-export.png`, `export-literal-editor.png`;
- `save-revision.png`, `saved-revision.png`;
- `verification.json` — saved data assertions and the tested binary SHA-256.

The virtual session was stopped after testing. These checks do not claim native
file-portal coverage or Windows execution.
