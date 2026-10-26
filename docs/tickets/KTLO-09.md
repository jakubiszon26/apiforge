# KTLO-09: Orphaned code audit — symbols reachable only from the legacy window

- Size: S · Risk: none (docs only; nothing is removed)
- Issue: [#12](https://github.com/jakubiszon26/apiforge/issues/12)

## Goal

List every item in `apiforge-desktop` (and other crates) that is reachable
**only** from the legacy workspace window, so a later sunset ticket
(KTLO-06, `#9`) can remove it deliberately. This ticket removes nothing.

## What "the legacy window" means

`main.rs` opens one of two front windows:

- `folder::FolderApp` — the default (folder projects, managed startup,
  `--folder`, and `--screenshot … --folder PATH`); the window the product is
  built around now.
- `app::App` — the legacy workspace window, reached only through
  `Opening::Workspace` / `Front::Workspace` in `run_window`, i.e. the saved
  JSON workspace (`--legacy`), a single `--project FILE` document,
  `--smoke-test FILE`, `--screenshot` without `--folder` (with `--project FILE`
  or the built-in default workspace), and the managed-startup fallback that
  still shows a migrated workspace.

An item is **legacy-only** when every reference to it, outside its own module
and that module's tests, lives in the legacy cluster below or in these legacy
launch arms of `main.rs`. `--check` opens no window and is not legacy.

Consequence for the sunset: the CI native probes `--smoke-test
fixtures/workspace.json` and `--screenshot test-results/native-window.png`
(`.github/workflows/rust.yml`, both runners) exercise the **legacy** window.
Removing it needs a folder-window replacement for those probes first.

## Method

Static reference analysis over `crates/apiforge-desktop/src`:

1. Classified every top-level module by whether `folder/`, the non-legacy
   arms of `main.rs` (`--folder`, managed startup, `--check`,
   `--screenshot … --folder`) or any shared module references it.
2. For shared modules, enumerated the `pub`/`pub(crate)` items and grepped
   each one's references across the crate, keeping those whose only callers
   are in the legacy cluster.
3. Verified every candidate by reading its real reference sites, because a
   name-only grep that excludes a module's own directory reports
   module-internal callers as "legacy-only" by mistake (this removed
   `inspector::drafts::set_owner`, which `inspector/fields.rs` also calls, and
   `dialogs::AssignVariableDialog`, which `folder/runs.rs` uses).

The dynamic cross-check the issue allows — deleting the legacy modules and
reading the compiler's dead-code list — was **not** run: neutralising the
legacy launch means rewriting the `Opening`/`Front` plumbing and three
`Arguments` arms in `main.rs`, which risks an uninformative build. The static
pass below is therefore conservative: it names what is confidently
legacy-only and may miss a deeply shared-looking item. Re-run the dynamic
check inside the KTLO-06 removal branch, where the rewrite is the work anyway.

## Legacy cluster — whole modules (every item is legacy-only)

The legacy window and the modules nothing else reaches. `folder/` references
none of them; `main.rs` references `app::{App, Source, default_workspace}`
only in the legacy launch arms named above.

| File | Items | Used only by legacy |
| --- | --- | --- |
| `app.rs` | the window itself: `Source`, `LayoutMode`, `layout_mode`, `App` (`new`, `select_node`, `has_document`, `prepare_close`, `save_pending`, `close_save_error`, `abort_close_after_save_error`, `ui`), `default_workspace` — 13 `pub` items, ~4710 lines | yes |
| `editor.rs` | `Draft`, `BlockClipboard`, `Editor` and its 31 methods (`new`, `workspace`, `graph`, `apply_graph`, `apply_scenario`, `apply_component`, `undo`, `redo`, `switch_scenario`, `switch_project`, `enter_component`, …) — 34 `pub` items | yes — referenced only by `app.rs` |
| `persistence.rs` | `SaveStatus`, `LoadResult`, `Saver` (`spawn`, `save`, `status`, `reload`, `take_loaded`, `flush`) — 10 `pub` items | yes — referenced only by `app.rs` |
| `navigator.rs` | `Navigator` (`open`, `is_open`, `show`) — 4 `pub` items | yes — referenced only by `app.rs` |
| `library.rs` | `Action`, `Example`, `Library`, `Context`, `Library::show` — 5 `pub` items | yes **today** — referenced only by `app.rs` and the legacy-only `dialogs/component.rs`; see note |
| `dialogs/component.rs` | `ComponentDialog` and its methods | yes — `ComponentDialog` is constructed only in `app.rs` (and `app_regressions.rs`) |
| `app_regressions.rs` | the legacy window's regression tests (`#[path]`-included from `app.rs:4709`) | yes — test code for `app.rs` |

Note on `library.rs`: it is legacy-only *at the time of this audit*, but
KTLO-05b (`#8`) deliberately brings the block-library part (`show_blocks`,
`Context`, the `AddNode`/`AddOperator`/`SelectNode` actions) into the folder
window. After KTLO-05b, `library.rs` is shared and only its workspace-
navigation actions — `SwitchProject`, `NewProject`, `SwitchScenario`,
`NewScenario`, `OpenExample`, `Example`, and the navigation half of
`Library::show` — stay legacy-only. A sunset after KTLO-05b should treat
`library.rs` as **partly** legacy, not whole.

## Shared modules — individual legacy-only items (partly)

These modules are used by the folder window; only the listed items are
reachable solely from the legacy cluster.

| File | Item | Used only by legacy | Shared part that stays |
| --- | --- | --- | --- |
| `canvas/mod.rs` | `Canvas::visible_center` (`pub`) | yes today — only `app.rs:1712`; KTLO-05b will call it for click-to-add, after which it is shared | the rest of `Canvas`/`Command` |
| `canvas/mod.rs` | `Canvas::debug_viewport` (`pub(crate)`) | yes — test helper used only by `app.rs` tests | — |
| `report.rs` | `ReportPanel::debug_resize_handle` (`pub(crate)`) | yes — test helper used only by `app.rs` tests | `ReportPanel` is shared (`folder/runs.rs`, `folder/report`) |
| `files.rs` | `Files`, `Outcome`, and `Files::{new, busy, poll, import, export, export_report}` | yes — `Files` is constructed only in `app.rs` | `read_document` (free fn) is shared through `main.rs` `--check`; its other callers (`--project`, `--smoke-test`, `--screenshot --project`) are legacy arms |
| `buffers.rs` | `Buffer` (incl. `blocks_the_board`, `apply_in`, `errors`), `Buffers` and all its methods (`text`, `applied`, `get`, `get_mut`, `blocks_the_board`, `typed`, `discard`, `restore`, `drafts`, `apply`), `Draft`, `Apply`, `State`, `print` | yes — referenced only by `app.rs` and `app_regressions.rs` | `APPLY_DELAY` (`folder/analysis.rs`, folder tests) and `Context` (`inspector/drafts.rs`, `inspector/fields.rs`) |

## Not legacy-only (checked, kept off the list)

To record what was ruled out: `board_edit`, `dialogs` (except
`component.rs`), `dialogs::AssignVariableDialog`, `dialogs::environment`,
`dialogs::settings`, `inspector` (incl. `drafts::set_owner`), `code`,
`canvas` (apart from the two items above), `preferences`, `theme`, `widgets`,
`capture`, and `dnd` are all reached by `folder/` or by the non-legacy arms of
`main.rs`. In other crates, `apiforge_runner` (`folder/runs.rs`),
`WorkspaceStore` and `JournalStore` (folder managed-startup migration and
`folder/activation.rs`) are shared, so no other-crate item is legacy-only —
the migration path keeps the JSON-workspace storage live.

## For the sunset ticket (KTLO-06 / #9)

Removing the whole modules (`app.rs`, `editor.rs`, `persistence.rs`,
`navigator.rs`, `app_regressions.rs`, `dialogs/component.rs`) and the legacy
launch in `main.rs` is the bulk of the work; it also strands the shared-module
items above, which then become plain dead code the compiler will flag. Do
`library.rs` only after KTLO-05b has taken the block library, and keep
`files::read_document` (for `--check`) and `buffers::{APPLY_DELAY, Context}`.
Replace the two CI native probes that open the legacy window before removing
it (see "What the legacy window means").
