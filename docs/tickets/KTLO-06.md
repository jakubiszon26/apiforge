# KTLO-06: Legacy JSON mode sunset — parity list and deprecation notice

- Size: S (docs) · Owner review: 20 min · Risk: `removal`
- State: Awaiting acceptance (issue #9) — the owner approves the parity list
  by reading the pull request; that approval is the acceptance
- Analysis model: Fable · Implementation model: — (documentation only)
- Owner decision of 2026-10-26 (on #9): write the parity list and the
  deprecation notice now; removal stays a later ticket. Reason: the list is
  what the owner approves, and it must exist before the folder mode can be
  measured against it.

## Goal

The legacy JSON workspace window (`--legacy`, `crates/apiforge-desktop/src/app.rs`)
is retired once the folder mode does everything the owner used it for. This
ticket says, capability by capability, what the folder mode has today and what
it still lacks, and states in the README and the roadmap the condition under
which the legacy window is removed.

## What the owner will see

- New: a deprecation notice in `README.md` and `docs/roadmap.md` naming the
  removal condition, and the list below.
- Changed, removed or hidden, migrated, default behaviour: nothing. No code
  changes; `--legacy` and the managed-start fallback behave as before.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

There is no acceptance test: the ticket changes documentation only, so rule 3
does not apply. Acceptance is the owner's reading of the list:

1. Every visible capability of the legacy window is a row below, with its
   state in the folder mode and the code that shows it.
2. The owner agrees with each **present**, **partial** and **absent**, or
   sends the row back.
3. The removal condition in the README and the roadmap can be read off the
   list.

## Not in scope

Removing, hiding or changing anything (the later `removal` ticket); the
migration of the owner's workspace (KTLO-03, #5); closing any gap the list
names.

## The parity list

**present**: the capability exists in the folder window, in the same or in
another form (named when it differs). **partial**: exists with a named gap.
**absent**: no equivalent. **n/a**: has no meaning in a folder project. The
legacy column cites `app.rs` line numbers unless another file is named; the
folder column cites `crates/apiforge-desktop/src/folder/`. Every row was
settled from the code at `395e228`; none is guessed.

### Workspace and navigation

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 1 | Several projects in one window; switch project (`library.rs:83`, `library_action` 1670) | present | One project per window; Project → Open Folder… and Recent projects (`navigation.rs:192`); switching protects unsaved work first (`navigation.rs:278`) |
| 2 | New project (`new_item_dialog` 2728) | present | Open Folder… on a folder without a project shows the start screen: name, register existing `.afs` files, Initialise project (`mod.rs:708`) |
| 3 | Scenario list, switch scenario, New scenario (`library.rs:109`) | partial | Explorer tree and tabs in Code (`explorer.rs:311`, `chrome.rs` `tab_strip`); New scenario / New component in the explorer menu (`explorer.rs:871`). In Diagram and Split there is no file list, only the tab strip and Ctrl+Tab: `explorer_panel` is drawn in `Mode::Code` alone (`mod.rs` `ui`). KTLO-11 (#14) adds a Files panel in Diagram |
| 4 | Examples: two demo scenarios opened into the workspace (`library.rs:246`, `open_example` 1721) | absent by decision | Left out of the folder panel (KTLO-05a decision 2: it would write files into the owner's folder). A first start creates the shop demo project (`activation.rs:241`); the tickets demo exists only after migrating a workspace that had it (legacy `default_workspace` 2847 holds both) |
| 5 | Find a step, Ctrl+K: search the board by name or type, select and reveal (`navigator.rs`, `editor_bar` 1982, `shortcuts` 2556) | absent | Nothing in `folder/` opens a navigator or binds Ctrl+K; the Add step popup's "Find an action…" searches blocks to add, not blocks on the board (`canvas/menu.rs:228`) |
| 6 | Scenario title and description in the header; project / Scenarios or Component editor line (`scenario_header` 1803) | present | Header shows the project name and a Read-only badge (`chrome.rs` `header`); the board header shows the document name (KTLO-12); the description is the `description "…"` statement of the source (ADR 0005, `parser.rs:214`) |
| 7 | Component editor: breadcrumb of nested drafts, Save revision, Back without saving (`breadcrumb` 2066, `scenario_header` 1803) | present | A component is its own document and tab (`open_component`, `diagram.rs:348`); Publish saves it and publishes the next immutable revision (`chrome.rs` `header`, `publication.rs:17`); Back without saving is Close tab → Discard (`close_tab_prompt`, `chrome.rs`) |
| 8 | Save status line (Saved, Save failed, Session only) with Retry save; autosave (`sidebar` 1586, `save_if_changed` 1307) | present | Explicit Save (Ctrl+S) and Save all (Ctrl+Shift+S) in the header; the PROBLEMS caption shows Saved / Unsaved changes / Unsaved layout / Read-only (`code_view.rs:304`); unsaved work reaches local recovery after 1 s (`RECOVERY_DELAY`, `mod.rs:72`). Explicit saving instead of autosave is a decided Stage II default (ADR 0012) |
| 9 | Undo and Redo buttons in the editor bar (`editor_bar` 1982) | partial | Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z only (`shortcuts`, `mod.rs:939`); no buttons |
| 10 | Code panel toggle, Ctrl+E, remembered in preferences (`toggle_code_panel` 687) | present | Code / Diagram / Split tabs; Ctrl+E swaps Code and Diagram (`workbench.rs`); the mode is remembered per project in the view state |
| 11 | Show / Hide Inspector while keeping the selection (`editor_bar` 1982) | partial | The inspector opens with one selected block and Close Inspector clears the selection (`inspector_panel`, `diagram.rs:381`); there is no hide that keeps the selection. Below 1440 px in Split neither window shows it |
| 12 | Compact layout below 1080 × 660 (`layout_mode` 67) | present | The window is laid out and tested at 960 × 540 (ADR 0013, `acceptance_tests.rs`) |

### Board and block library

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 13 | Block library: search, Blocks / Variables / Operators, Add variable, project components, click or drag to add (`library.rs:142`) | present | The same `show_blocks` in a left panel in Diagram and in Split at 1440 px and wider (`library_panel`, `mod.rs:1035`; KTLO-05b), width remembered per project (KTLO-05c), collapsible to a rail with Ctrl+B (KTLO-17); below 1440 px in Split the Add step popup carries Variables and Operators (KTLO-05d, `canvas/menu.rs:269`) |
| 14 | The board: Add step popup, connect, move, delete, copy / paste / duplicate, Tidy up, wire style and bridges, zoom and fit, overview, shortcuts help, variable proxies, open a component's steps (`canvas/mod.rs`; `board_command` 2343) | present | The same `Canvas`; every `Command` is applied by `board_command` (`diagram.rs:160`); wire style and bridges are saved to preferences (`diagram.rs:201`) |
| 15 | Save selection as component: name, description, public arguments, outputs, contracts (`Command::SaveSelectionAsComponent` 2436, `dialogs/component.rs`) | absent | The command only notifies "Saving a selection as a component is not available in folder projects yet." (`diagram.rs:309`). A component starts as a new document (`component_template`, `mod.rs:1355`) and declares its inputs in code |
| 16 | Scenario / After test phase tabs (`editor_bar` 1982) | present | Main / Cleanup in the board header (`diagram.rs:706`) |

### Inspector

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 17 | The inspector: names, arguments and bindings, method / URL / headers / query / body with completion, timeout, body format, checks, contract and sample response, output data with field search, Check and To variable on a field, Open component, Update to revision, variable mark (`inspector/`; `inspector` 2197) | present | The same `Inspector` in a right panel in Diagram, and in Split at 1440 px and wider (`inspector_panel`, `diagram.rs:381`); every `inspector::Action` is handled (`diagram.rs:455`) |
| 18 | Open in code reveals the block's statement (`inspector_action` 695) | present | `reveal_in_code` (`diagram.rs:574`) opens a view with code and places the caret |
| 19 | Unfinished URL, header, query and body text kept across restarts (`save_drafts` 1325, `inspector/drafts.rs`) | present | Field drafts in local recovery (`FieldRecovery`; `reload_field_drafts` `mod.rs:448`, `sync_field_drafts` `diagram.rs:508`) |

### Code panel and drafts

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 20 | The scenario or component as code beside the board; typing becomes a document edit after 250 ms; a block added in code is placed by the layout (`code_panel` 789, ADR 0007) | present | Code and Split; the `.afs` file is the source and the board follows the analysed source (`code_view.rs:36`, `analysis.rs`, ADR 0012) |
| 21 | Invalid code holds the board read-only: banner, Show code, Discard code changes (`code_banner` 1016, `ui` 1541) | present | "This diagram is read-only" with Show code (`diagram.rs:64`); the errors are listed under PROBLEMS with line and column and an Apply fix (`diagnostics_panel`, `code_view.rs:304`), which the legacy panel lacks. The code is the file, so "discard" is Close tab → Discard, back to the saved version |
| 22 | Code drafts come back after closing or a crash; one written against a changed document is marked stale: Keep editing it / Discard it (`restore_drafts` 2864, `stale_banner` 990) | present | Recovery under `.apiforge/local/`; a draft over a changed file waits with Keep unsaved work / Discard it (`draft_banner`, `code_view.rs:258`); drafts of documents gone from the project are reported with Discard (`notices`, `chrome.rs`) |
| 23 | "No code view" for a scenario the language cannot write (`code_panel` 789) | n/a | Every folder document is its source file |
| 24 | Caret and board selection kept in step (`sync_selection` 923) | present | `sync_selection` (`code_view.rs:174`) |
| 25 | Completion, hover and diagnostics in the editor (`code.rs`) | present | The same `CodeEditor` (`code_view.rs`), with shapes from the last run and the large-source rules of the development guide |

### Runs and reports

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 26 | Run test; Cancel run (`scenario_header` 1803, `start_run` 1229) | present | Run / Cancel run in the header (`chrome.rs` `header`, `request_run` `runs.rs:79`). The run takes the current text, saved or not; the legacy ran the saved document |
| 27 | API / Simulation mode toggle (`scenario_header` 1803) | present | Run simulates; Run options → Run live requests (`chrome.rs` `header`) |
| 28 | Run up to selected block; Run only selected step (`RunKind`, `start_run` 1229) | absent | The folder always requests a full run (`target_node_id: None, single_node_id: None`, `runs.rs:164`) |
| 29 | Run with invalid code asks "Run the last valid version" (`run_with_errors_dialog` 538) | present | "Run the last valid version?" (`run_prompt`, `runs.rs:217`) |
| 30 | A live run that cannot be journalled is announced before it starts (`scenario_header` 1803) | present | "Run without a journal?" prompt (`runs.rs:227`) |
| 31 | Run results panel: steps per phase, inputs / outputs / request, expected and actual, status, stale marker, Check and To variable on a field, select a step, Continue remaining steps / Stop on error, keyboard-resizable (`report.rs`; `report_action` 2104) | present | The same `ReportPanel` under the tabs (`run_report`, `runs.rs:255`); every `report::Action` is applied (`runs.rs:295`). Filling the window is KTLO-19 (#44) |
| 32 | Export the report as JSON with masked secrets (`files.rs:89`) | present | Export writes the run as JSON (`runs.rs:319`). The runner publishes only redacted snapshots (`apiforge-runner/src/service.rs:285`); the legacy export redacts once more on top (`files.rs:91`), the folder export does not — a finding, not a visible difference |
| 33 | Interrupted live runs listed at the next start: Mark as handled / Remind me later (`interrupted_runs_dialog` 581) | present | `journal_prompt` (`runs.rs:477`) for the project's journal, and Run options → Review legacy runs for the journal migrated from the JSON workspace (`runs.rs:436`) |

### Settings and environment

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 34 | Environment: base URL, variables, session secrets, with the host shown on the sidebar button (`dialogs/environment.rs`; `sidebar` 1586) | present | Project → Test environment…, the same dialog (`settings.rs:15`); Apply marks the project file unsaved and Save all writes it. The host label on the sidebar has no equivalent |
| 35 | Settings: project name, scenario name, description, After an error, scenario parameters (JSON), data contracts (`dialogs/settings.rs`; `project_menu` 1755) | present | Project → Project settings… edits the name and the contracts (`SettingsDialog::project`, `settings.rs:5`); scenario name, `description`, `on_error stop \| continue` and `param` are statements of the `.afs` file (ADR 0005, `parser.rs:214–241`); After an error is also set from the report panel in both windows |
| 36 | Import project… (JSON workspace or project) and Export project… (JSON without secrets) (`project_menu` 1755, `files.rs:56,73`) | partial | Import files… takes `.afs` sources into the folder (`explorer.rs:896`, `file_prompts.rs:107`); a JSON workspace enters folder mode only through the one-time migration at managed start (`activation.rs:61`, KTLO-03). No export: the folder is the exchange format; unsaved work can be exported when closing fails (`file_prompts.rs:40`) |

### Storage, recovery and closing

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 37 | Backup restore at start, only after confirmation (`storage_dialog` 494) | present | "Unsaved work from an earlier session is in the recovery backup" with Restore backup (`notices`, `chrome.rs`); an interrupted file operation opens the transaction recovery review before anything loads (`transaction_view.rs`) |
| 38 | The workspace file changed outside the window: saving stops, Reload and discard local changes / Continue without saving (`storage_dialog` 494) | present | A changed project file pauses saving with Reload project (`registry_notice`, `chrome.rs:345`); a changed source reloads by itself or opens Review conflict… with exports of both versions (`conflict_view.rs`) |
| 39 | A second window opens read-only and says why (`persistence.rs`) | present | Read-only badge in the header; export still possible (`chrome.rs` `header`) |
| 40 | Closing waits for the run and its cleanup; a failed save keeps the window open with Retry save (`prepare_close` 334) | present | Close waits for storage and the run; "Unsaved work is not protected": Keep editing / Try again / Export and close / Discard (`close_failed_prompt`, `chrome.rs`) |

### Launch and command line

Entry points rather than things on screen, listed because they decide what
the removal touches.

| # | Legacy window | State | Folder mode |
| --- | --- | --- | --- |
| 41 | `--legacy` opens the saved JSON workspace; managed start falls back to it when migration is refused or the workspace is locked (`main.rs:167,227`, `activation.rs:61`) | partial | The fallback is decided away, not yet implemented: a refused migration reports and stops, and `--legacy` stays an explicit request (KTLO-03 owner decision 1 on #5) |
| 42 | `--project FILE` opens a JSON document for the session only; `--smoke-test FILE` and `--screenshot --project FILE` drive this window in CI (`main.rs`; `rust.yml:106–141`) | absent | `--folder` always opens the project itself (development guide); both CI runners' native probes open the legacy window (KTLO-09). A folder replacement for them must land before the removal |

## Removal condition

The legacy window is removed, in its own `removal` ticket the owner approves,
when all of these hold:

1. Every row above marked **partial** or **absent** is **present**, or struck
   by an owner decision recorded on its ticket. Today that is rows 3
   (KTLO-11), 5, 9, 11, 15, 28, 36, 41 (KTLO-03) and 42; row 4 is already
   struck (KTLO-05a).
2. KTLO-03 (#5) is closed: migration of the owner's workspace works, and a
   refused migration reports and stops instead of opening the legacy window.
3. The CI native probes (`--smoke-test`, `--screenshot --project`) have
   folder-window equivalents on Linux and Windows.

## Questions for the owner

Each is a removal-or-keep decision on a gap; the agent recommends and stops.

1. **Rows 15 and 28** (save selection as component; partial runs): required
   before removal, or struck? Recommended: required — one is how a component
   is born from a working scenario, the other how a single step is debugged.
2. **Row 5** (Find a step): required in some form, or struck? Recommended:
   required; KTLO-11 adds file navigation, not a search over the board.
3. **Rows 9 and 11** (Undo / Redo buttons; hide the inspector keeping the
   selection): recommended struck — the shortcuts and reselecting do the job.
4. **Row 36**: is a JSON export of a folder project needed? Recommended: no;
   the folder and Git are the exchange format.
5. **Row 4**: a first start creates the shop demo only. Recommended: leave.

Findings outside the list, for separate tickets: the folder report export
relies on the runner's redaction alone (row 32); the README's "known
regressions" bullet on KTLO-05 is stale now that 05b–05d and KTLO-17 merged.

## Technical appendix

For the removal ticket: `docs/tickets/KTLO-09.md` names what only the legacy
window reaches (`app.rs`, `editor.rs`, `persistence.rs`, `navigator.rs`,
`app_regressions.rs`, `dialogs/component.rs`, the navigation half of
`library.rs`, `files::{Files, Outcome}`, most of `buffers.rs`) and what stays
(`files::read_document` for `--check`, `buffers::{APPLY_DELAY, Context}`,
`WorkspaceStore` and `JournalStore` for migration). Re-run its dynamic
dead-code check inside the removal branch. `fixtures/workspace.json` stays:
smoke item 9 migrates it and `--check` reads it.

## Actual

Changed lines: 207 (`git diff --stat origin/main...HEAD`), against the S
estimate of ≤ 300. Rows settled from the code: 42 of 42. Rounds: —.
