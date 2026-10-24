# Stage II P1–P4: native interactive QA

- Date: 2026-10-24
- Tested main: `8fafaaec1a8806b55495280eacafa627a14df3f4`
- Result: **one confirmed diagram-layout defect**; the exercised storage, recovery, document-isolation and conflict paths passed.
- Product code was not changed. The confirmed defect is tracked as [BUG-035](../../../knows-bugs.md#bug-035--p2--newly-analysed-folder-project-cards-overlap).

## Confirmed defect: newly analysed cards overlap

Severity: medium. Reproduced in both Diagram and Split at 100% zoom, and after closing/reopening the project.

1. Open a fresh folder with `apiforge-desktop --folder PATH` and initialise it.
2. Create a scenario and replace its source with:

```afs
scenario "QA Alpha" {
  alpha = 7
  beta = alpha + 3
}
```

3. Wait for analysis, then choose Split or Diagram.

Expected: both cards and their ports remain visible without overlapping.
Observed: `beta` begins only 120 canvas units below `alpha`, covering its lower portion and ports. Saving and reopening preserves the overlap.

Evidence: [first Split view](p1-p4-native/split-good.png), [Diagram after restart](p1-p4-native/overlap-after-restart.png).
The actual saved checkpoint placed `alpha` at `(0, 0)` and `beta` at `(0, 120)`.

Code inspection explains the mismatch: `FolderApp::open` uses `ProjectSession::open` (`crates/apiforge-desktop/src/folder/mod.rs:169`), which uses the default `FixedPlacer`. That placer assumes every card is 240×96 (`crates/apiforge-project/src/layout.rs:37`), although the native renderer draws larger cards with height depending on content. A desktop placer needs the rendered dimensions, including when creating a new folder project.

## Exercised GUI checks

| Check | Result and evidence |
| --- | --- |
| Opening an uninitialised folder | PASS: initial screen appeared and the project directory stayed empty before Initialise was clicked. |
| Initialise and empty project | PASS: manifest and gitignore created only after the explicit action; empty document list rendered. |
| New documents | PASS: created QA Alpha and QA Beta through the menu; two distinct registered paths/tabs. |
| Code → diagram | PASS: two values and a POST request appeared after source analysis. Layout defect above applies to the two-value scenario. |
| Mode and tab switching | PASS: Code/Diagram/Split, Ctrl+E and Ctrl+Tab retained the edited documents. |
| Hidden invalid document isolation | PASS: Alpha analysed as invalid while Beta remained a valid, editable request diagram. [Beta](p1-p4-native/independent-valid.png), [Alpha](p1-p4-native/invalid-alpha.png). |
| Invalid-source protection | PASS: Alpha showed its last-valid two-node model, an explicit read-only banner and disabled Add step. |
| Located diagnostics | PASS: incomplete trailing source showed line/column information; trailing unattached comments showed a warning. |
| Exact save, including invalid source | PASS: Ctrl+S wrote the editor source byte-for-byte, including unfinished suffix/comment. Compared clipboard read-back with the actual `.afs` bytes. |
| Saved invalid source after restart | PASS: exact source returned; its last-valid model remained available. [Restart](p1-p4-native/restart-invalid.png). |
| Text undo and redo | PASS: a pasted comment was undone to the exact baseline; redo returned it and recovery retained it. |
| Text history after mode/tab switches | PASS: pasted a comment in Beta, switched to Alpha and back, toggled Diagram and Code, refocused code, then undid back to the exact original bytes. Multiple undo presses were used because intermediate caret/selection states also occurred. [Baseline](p1-p4-native/history-base.txt), [read-back](p1-p4-native/history-undo3.txt), [screen](p1-p4-native/history-undo3.png). |
| Unsaved text recovery | PASS: normal native close/reopen retained the exact unsaved Alpha source. Recovery JSON text matched the prior editor read-back. |
| Invalid inspector URL | PASS: `{{unfinished` stayed a field draft, showed a validation error, and did not replace the model URL `/qa`. [Draft](p1-p4-native/field-draft.png). |
| Close-tab Discard | PASS: close offered Save/Discard/Cancel; Discard closed Beta and reopening its Inspector showed `/qa`, without the invalid draft. [Verified widget](p1-p4-native/field-discard-verified.png). |
| Field-draft recovery | PASS: `{{recover_url` survived native close/reopen and reappeared in the actual Inspector URL widget with its validation error. [Recovered field](p1-p4-native/field-recovered.png). |
| 960×540 client area | PASS: actual X11 client geometry checked; Code, Diagram and Split rendered, mode/save/tab controls remained reachable. Narrow Split omitted the Inspector as designed; Diagram Inspector content scrolled. [Code](p1-p4-native/minimum-code.png), [Diagram](p1-p4-native/minimum-diagram.png), [Split](p1-p4-native/minimum-split.png). |
| Wide Split | PASS: code, diagram and Inspector visible together at 1600×900. [Wide view](p1-p4-native/split-wide.png). |
| Second instance | PASS: visible Read-only label/banner and disabled Save controls. An in-memory edit plus Ctrl+S and explicit close/discard left hashes of the manifest, both sources, diagram and shared recovery unchanged. [Read-only](p1-p4-native/read-only.png), [save attempt](p1-p4-native/readonly-save.png). |
| Read-only close protection | PASS: recovered unsaved work prevented silent close; Keep editing/Try again/Discard options appeared. Explicit discard closed the second window without changing shared recovery. [Prompt](p1-p4-native/readonly-close.png). |
| Same-length external edit | PASS: changed saved Alpha `7` to `8` outside the app. Ctrl+S refused with a conflict; disk SHA-256 stayed equal to the external version. [Save conflict](p1-p4-native/external-conflict.png). |
| Discard after external edit | PASS: Discard refused the changed baseline, kept the tab and local source, and did not overwrite the external file. [Discard conflict](p1-p4-native/conflict-discard.png). |

## Supporting automated checks

Commands run against the same main:

```text
cargo build -p apiforge-desktop --locked
cargo test -p apiforge-storage --lib --locked project:: -- --test-threads=1
cargo test -p apiforge-project --test migration --locked -- --test-threads=1
cargo test -p apiforge-project --test recovery --test session --locked -- --test-threads=1
```

Build passed. Tests: **152 passed, 0 failed**; one parent-launched storage helper ignored as intended; 38 unrelated storage tests filtered out. Breakdown: storage 111, migration 13, recovery 13, sessions 15. Storage coverage includes native process interruption/locking, transaction fault injection, checkpoint provenance, trash and exact-source recovery. These automated results are separate from the GUI observations above. Formatting, clippy and the complete workspace suite were not rerun in this QA pass.

P3 migration has no GUI entry point at this stage. Its 13 tests verified fixture migration, drafts, catalogue activation, occupied destinations, changed input and interrupted publication/resume; no claim of interactive GUI migration acceptance is made.

## Environment and limits

CachyOS Linux, kernel `7.2.10-1-cachyos`, x86_64. Debug binary SHA-256:
`ef63991d091b5eafcfef83a109e36189c0a85a76150775ccfc1145f4d5611a54`.

The real eframe binary ran through `--folder` on a dedicated virtual KWin output with its own D-Bus session and Xwayland. Successful input used KWayland FakeInput (keyboard, pointer and buttons); source was pasted through a selection owner on that isolated X server. Screenshots were read from the actual native window using Python Xlib/Pillow. This was not an egui-only harness. Client sizes exercised: 1600×972, 1600×900, 960×540.

Project/config/data/runtime and clipboard were isolated under `/tmp/apiforge-p1p4-qa`. The original user's workspace was not opened. An initial compositor attempt failed importing dmabufs; the working session disabled Xwayland glamor and used software-compatible settings. Failed setup/launch attempts are not counted as product defects. No GPU performance claim is made. The helper and app processes were stopped after testing.

This is bounded Linux regression evidence, not full P1–P4 or P7 release acceptance. Runs/reports and component publication in folder windows belong to P6; Explorer/polling belongs to P5. Windows, direct Wayland input, accessibility, native file dialogs, crash recovery in the interactive GUI, the full component/revision workflow, Save All, graphical undo/redo and large-file performance were not exercised in this pass. The normal legacy startup path was not retested here.
