# Desktop bug fixes — verification, 2026-10-24

All 17 findings in [the original report](../../../../knows-bugs.md) have working-tree
fixes. Original evidence remains a record of revision `854b44f`; this report covers
the subsequent changes.

| Findings | Change and regression coverage |
| --- | --- |
| BUG-001 | Pending text survives intervening frames and enters the draft set before debounce. An actual first paste through egui and the native window retains `answer = 42`. |
| BUG-002, BUG-007 | Pending, invalid and unresolved code blocks document commands, undo/redo, library additions, inspector changes, result actions and component saves. A dialog opened earlier cannot commit an obsolete graph. The native Add step control stays disabled. |
| BUG-003, BUG-010 | Completion keys belong to the focused editor; blur dismisses the popup. Its anchor updates with the current caret geometry. Tests cover two editors, manual completion and diagnostics moving the editor; native retests confirm positioning and dismissal. |
| BUG-004 | The last valid board remains legible. Its status and recovery actions remain visible when Code is closed; mutation controls reflect the lock. Reviewed in the native window with an inspector. |
| BUG-005 | Code and inspector share the auxiliary-panel slot on narrow windows; code width is capped to reserve board space. Automated resize from 1600×1000 to 960×540 and native retests show a useful board without toolbar/inspector overlap. The Inspector toggle switches to the inspector at narrow widths. |
| BUG-006 | Components use the same placement of fresh blocks as scenarios. Regression checks all new cards against existing cards and verifies that existing positions stay unchanged. |
| BUG-008 | Saving and restoration include inactive projects. Regression saves two projects and restores both with the second project active. |
| BUG-009 | Run settles pending typing before taking its snapshot. Immediate Run after changing `value = 1` to `value = 2` now fails the current assertion and produces current results. |
| BUG-011, BUG-013 | Unfinished URL, header, query and body fields use the workspace draft store and its secret checks. Tests persist invalid URL/body text, restore it into a fresh egui context and expose conflicts after external field edits. Conflicts offer keep/current-field actions. |
| BUG-012 | JSON-body completion uses placeholder syntax accepted by the body parser. Regression verifies no bare-expression offer and successfully parses a completed `{{param.quantity}}` value. |
| BUG-014 | Code selection resolves main and cleanup graphs and changes phase in both directions. |
| BUG-015 | Reveal scrolls the enclosing code viewport to the target caret. A 100-line editor regression checks that an offscreen statement becomes visible. |
| BUG-016 | Stale drafts cannot apply before a choice; their base hash survives further writes. A valid stale draft leaves the newer value 2 unchanged across frames, saving and discard. Reloading a changed workspace also marks retained text as conflicting. Component drafts retain the saved revision as their base. |
| BUG-017 | Whole-scenario history stores both graphs independently of the active phase. Undo/redo tests restore both graphs and settings from cleanup. Native Undo also restored main 1 and teardown 2 together. |

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: **515 passed, 0 failed, 1 pre-existing ignored test**.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
- Native debug application, isolated data/preferences, Xvfb/X11, Mesa Lavapipe:
  first paste, invalid-code lock/recovery, completion anchor/blur, whole-scenario
  Undo on cleanup, and resizing with code/inspector already open.

The HTTP transport DNS-error test now injects a failing resolver rather than
assuming the host's DNS respects `.invalid`. It still exercises reqwest's real
connector error path and runs without external DNS access.

Native captures:

- [Readable invalid board and disabled inspector](invalid-board.png)
- [Recovery after closing Code](hidden-code-recovery.png)
- [960×540 window with Code open](minimum-window.png)
- [Whole-scenario Undo while cleanup is active](undo-both-phases.png)

The complete workspace suite includes the loopback HTTP tests, run outside the
network-restricted sandbox. Native retests used a session-only workspace and sent
no API requests. Remaining findings were verified by focused regression tests;
this is not a full native replay of all 17 original reproduction scripts, nor a
Windows/Wayland validation.
