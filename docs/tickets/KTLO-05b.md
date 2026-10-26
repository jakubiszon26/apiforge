# KTLO-05b: Block library beside the board — implementation

- Size: M · Risk: `visible UI`
- Issue: [#8](https://github.com/jakubiszon26/apiforge/issues/8)
- Analysis and acceptance criteria: `docs/tickets/KTLO-05a.md` (approved,
  option A; *Examples* left out)
- Implementation model: Opus 5.5

## Delivery

The ticket ships in parts so each stays reviewable and so one open question
does not block the panel.

### PR A — the panel (this PR)

The *Block library* panel beside the board in Diagram, and in Split at
`SPLIT_INSPECTOR_WIDTH` (1440 px) and wider. `Library::show` is split into
`show_blocks` (the Blocks / Variables / Operators sections and the project's
components), shared by the legacy sidebar and the new panel; the folder leaves
out the *Examples* entry via `Context::show_examples`. Clicking a block or
operator adds it at the board's visible centre, selected, and dirties the tab;
variables list the document's variable nodes and *Add variable* adds one. The
panel is disabled when the board is read-only. The empty-board hint names the
library only when it is in view (`canvas::Input::library_in_view`). The
custom-painted library item, operator and variable chips and the disclosure
headers get accessible names so the folder accessibility audit passes.

Criteria covered: 1, 2, 3, 4, 5 (panel half), 7, 8, 9.

### PR B — the popup sections (follow-up)

Criterion 5's second half: in Split below 1440 px there is no room for the
panel, so the *Add step* popup (`canvas/menu.rs::ActionMenu`) gains the
Variables and Operators sections. Kept separate to hold PR A within its size
class and because it is a distinct surface.

## Open decision — criterion 6 (panel-width persistence)

Deferred pending the owner. "The panel's width is restored per project with
the other panel sizes" means adding a `library` field to
`apiforge_storage::project::PanelSizes`. That struct has
`#[serde(deny_unknown_fields)]`, so a build without the field would reject a
workspace file written with it — a backward-compatibility change, which is the
owner's call (AGENTS.md rule 1). Options, with a recommendation, are on issue
#8. Within a session egui already remembers the resized width; only
cross-session, per-project persistence waits on this decision.

## Not in scope

As `docs/tickets/KTLO-05a.md`: scenario navigation in Diagram, keyboard-only
placement, the *Examples* entry, and any restyling beyond the existing panels.
