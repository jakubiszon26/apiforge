# Stage II P7f: source budgets, keyboard and accessibility, user guide

Branch: `feat/stage-ii-p7-performance`. Base: unmerged P7e `3c71e38`
(`feat/stage-ii-p7-portability`). Implementation: `e06f8e8`, `abc574f`, `74555c1`,
`c1cf1ce`, `70c967e`.

## Source budgets

The roadmap's analysis budget is parse, lower and validate 1000 lines in 5 ms. The
new `apiforge-project` `analysis` example measures it on a valid scenario of
993 lines, ten lines per step, and reports a valid 1 MiB scenario (977,657 bytes,
24,483 lines, 90 requests with pasted 260-row samples, under the 200-block limit).
The graph limit means a valid source reaches 1 MiB through sample and body data,
not through more blocks. Release measurements on this Linux host:

| Measurement | Result | Budget |
| --- | --- | --- |
| Parse + lower + validate, 1000 lines | 1.73 ms | 5 ms |
| Parse + lower + validate, 1 MiB | 62 ms | none (worker thread) |
| Analysis after an edit, 1000 lines | 3.1 ms | none |
| First analysis with automatic layout, 1000 lines | 5.2 ms | none |
| Analysis after an edit, 1 MiB | 114 ms | none (worker thread) |

Analysis runs on a worker after a 250 ms typing pause, so the frame budget is what
the UI thread must keep. A release probe of the real folder window found that an
idle code-view frame took 81 ms at 1 MiB (Split 132 ms, typing 89/213 ms). The UI
thread hashed the 1 MiB text several times per frame for dirty state, deep-copied
the model and both graphs into the code view, inspector and diagram, rebuilt
completion names from every block and sample, reparsed the whole source for the
statement under the caret, built full analysis inputs while typing paused, and
found binding sources with one ancestor walk per block.

The session now keeps its model behind an `Arc` and memoises the text's file state
behind its only setter. Views share the model; completion names and the caret's
statement are cached by model identity and source version; the analyser polls
tickets before building an input; and sources come from one dependants walk.
Above 256 KiB completion opens on Ctrl+Space and caret-to-board sync waits for the
edit's analysis, extending the existing plain-colouring rule into one documented
large-source mode.

`frame_budgets`, an ignored release test, opens real folder projects and averages
ten idle and ten typing frames with the caret on an assertion mid-source. Final
release results:

| Source | Mode | Idle | Typing | Budget (idle / typing) |
| --- | --- | --- | --- | --- |
| 1000 lines | Code | 0.6 ms | 3.4 ms | 16 / 16 ms |
| 1000 lines | Split | 3.0 ms | 5.7 ms | 16 / 16 ms |
| 1 MiB | Code | 4.9 ms | 26.7 ms | 16 / 33 ms |
| 1 MiB | Split | 8.6 ms | 32.3 ms | 16 / 50 ms |

A selected request with a 260-row sample measured 9.5 / 12.7 ms in 1 MiB Split
once the per-frame copies were gone. Typing at 1 MiB still relays out the whole
galley in egui; virtualised text layout would be the next step and is not needed
for these budgets. The 1 MiB limit tests that already existed (store, recovery,
CLI, editor layout and analysis) remain unchanged.

## Keyboard and accessibility

An AccessKit audit now runs over every Stage II screen and dialog at 960 × 540
and 1600 × 1000: Code, Split and Diagram, project settings, test environment,
close-tab, new document, rename, move, new folder, delete, the conflict banner and
review, transaction recovery and initialisation with a found source; every block of
the demo scenario in the inspector; and the Project, Run options and New menus
opened from the keyboard. It fails on any focusable or interactive node without a
name. Before this package text fields, choices, the code editor, the explorer's
keyboard target, panel resize handles, canvas hit areas, inline card fields and the
menus themselves had none; tooltips and visible labels are not names.

Keyboard tests check reading order from the project menu through mode switch,
tabs, explorer actions and tree to the editor; that Tab stays inside the settings
and environment dialogs and Escape cancels them; that Tab indents in the editor
while Escape then Tab leaves it; and that each menu opened with Space starts with
its named menu stop followed by its items. egui adds one empty stop when Tab wraps
past the last control; it is egui's behaviour and is recorded rather than patched.

Native Linux review used the release build, Xvfb, Mesa, XTest input and an
isolated D-Bus session with the GTK file portal (`XDG_CURRENT_DESKTOP=gnome`):

- Tab and Space [opened the Project menu](keyboard-project-menu-960.png) with a
  visible focus ring; Tab moved to [Open Folder…](keyboard-open-folder-960.png) and
  Enter opened the real [portal folder chooser](portal-open-folder.png). It now
  starts in the folder containing the current project; before, it opened GTK's
  empty Recent view. Selecting a neighbour [switched projects](opened-second-960.png).
- The 1 MiB project [opened](large-source-960.png), took typed text at its end
  and saved it exactly with Ctrl+S (977,693 bytes on disk, ending with the typed
  line). The unattached-comment warning appeared as expected
  ([960 × 540](large-source-saved-960.png), [1600 × 1000](large-source-end-1600.png)).
- Project settings opened from the keyboard; focus was visible on
  [buttons](keyboard-settings-close-960.png) and the
  [project name field](keyboard-settings-field-960.png), and Shift+Tab moved back.

This is not a screen-reader session: no AT-SPI bus ran, so Orca or NVDA output was
not observed. The names are verified in egui's AccessKit tree. Windows native UI,
Windows directory durability, native Wayland parenting and startup activation
remain open release gates. No layout algorithm or language implementation changed.

## User documentation

[Folder projects](../../../folder-projects.md) is the user guide: folder contents
and what belongs in Git, opening and switching, the window, saving and recovery,
conflicts, settings and secrets, runs, large sources, keyboard use and Git
practice. The README now links it and no longer says folder projects are
unimplemented; the development guide records the budget commands, the
large-source mode and how to extend the accessibility audit.

## Verification

Final verification of implementation `70c967e` with Rust 1.99.0:

- `cargo fmt --all -- --check` and workspace/all-target clippy with warnings
  denied passed.
- Serial all-target workspace tests under isolated XDG paths: **916 passed,
  0 failed, 3 ignored** (two process helpers and the release-only frame test)
  across 49 test binaries.
- The v1 fixture check passed; the isolated UI quality gate passed **10/10**.
- `analysis --strict` passed (1.68 / 5 ms; 1 MiB 61 ms) and `frame_budgets`
  passed with the results above.
- Release layout budgets passed with `--strict`: layered 0.31/20 ms, full routing
  24.44/30 ms, dragging 0.20/8 ms, smoothing 0.27/8 ms, placing 0.06/8 ms; random
  wiring 766 ms has no budget.

No language implementation changed, so the fuzzers were not rerun.
