# P1 completion: native interactive regression QA

- Date: 2026-10-24
- Tested implementation: `b67043e` on `feat/stage-ii-code-workspace`
- Result: all exercised checks passed; no new functional defect found in this pass
- Scope: current native desktop regression after P1 storage completion; P2 was not started

## Environment and interaction tool

Ubuntu 26.10, Linux `7.3.0-9-generic`; debug binary built with
`cargo build -p apiforge-desktop --locked`.
SHA-256: `2b30cdad73261440d86659781a4c400b1dd7d034738606be27358d32b1dff8ee`.

The real eframe application ran on a separate Xvfb X11 display (`:117`,
1600×1000×24). Keyboard/mouse input came from **xdotool**, text transfer/read-back
from **xclip**, and screenshots from Pillow's X11 capture. A standard
`WM_DELETE_WINDOW` client message tested normal close. This exercised the native
window and backend, not an egui-only test harness or screenshot-only smoke probe.

`WAYLAND_DISPLAY` was unset, `WINIT_UNIX_BACKEND=x11` and
`XDG_SESSION_TYPE=x11`. `XDG_DATA_HOME` and `XDG_CONFIG_HOME` pointed to separate
scratch directories under `/tmp/apiforge-p1-ui`. The application created its
English default demo workspace there. No user's workspace or clipboard was used.
The screenshot set below contains only this isolated test application.

A local `cargo run -p apiforge-demo-api --locked` server supplied
`http://127.0.0.1:4318/demo` for live HTTP tests. No third-party API was called.
Mesa emitted DRI3 acceleration warnings on Xvfb; painting, interaction and runs
continued successfully. This pass makes no hardware-GPU performance claim.

## Checks and observed results

| Check | Observed result |
| --- | --- |
| Startup and initial layout | Real 1600×1000 window rendered the default scenario, components and cleanup tab |
| Ctrl+E | Code panel opened; closing/reopening retained exact edited source |
| Valid code → diagram | Renamed scenario to `P1 native QA`, added `qa_probe = 7`; debounce updated title and graph to seven main nodes |
| Invalid code | Appending `broken = (` showed a located error and read-only board; last-valid seven-node model remained intact |
| Protected graph operations | Add-step/inspector controls were disabled; attempting Add step did not change saved nodes |
| Text undo/redo | Ctrl+Z restored exact valid text; Ctrl+Y restored exact invalid text, checked byte-for-byte through editor read-back |
| Run with invalid source | Explicit last-valid confirmation appeared before execution; no automatic run |
| Interrupted process and restart | After draft persistence, terminated the app and reopened it; recovered invalid source matched exactly |
| Second native instance | Read-only warning visible; an in-memory code edit did not change the workspace SHA-256 |
| Return to valid source | Repair removed error protection and restored graph editing |
| Simulation | Nine steps passed, including cleanup; result panel showed sample-response mode |
| Minimum 960×540 window | Resize retained visible Run, mode control, code, board and report controls; inspector collapsed and graph labels truncated for compact layout |
| Diagram → code | Inspector label edit produced `@label("QA graph renamed")` without changing the block handle |
| Model undo/redo | Ctrl+Z/Ctrl+Y with board focus removed/restored the graph edit; verified generated source |
| Stale report indicator | Editing the model after a run displayed that results came from an earlier graph version |
| Component editor | Opened Add to cart from an instance and edited its source |
| Invalid publication | Broken component source disabled Save revision; persisted components stayed identical |
| Publish revision | Added `qa_revision_probe = 11`, opened Save revision and confirmed Save component; revision 2 persisted with the probe |
| Revision isolation | Previous component objects remained identical, and the complete scenario including revision-1 instance bindings stayed unchanged |
| Live HTTP failure | Running before the local server was ready produced an error report with inner component steps, without a crash |
| Live HTTP success | Once the server was ready, nine steps passed; the final state had no unresolved journal file |
| External edit protection | Changed the scratch workspace's project name on disk, then edited code in GUI; saving stopped with explicit reload/session choices and external file hash stayed identical |
| Reload | Explicit reload showed `External disk QA` and retained the local draft as stale, with Keep/Discard choice; it did not apply that draft automatically |
| Discard stale draft | Discard restored the saved scenario name/source; external project name remained intact |
| Identity/layout retention | Compared saved node-ID → position maps before publication and after reload/discard; maps were identical |
| Project settings | Opened the native in-app settings dialog with the reloaded project/scenario values |
| Normal close and restart | Sent `WM_DELETE_WINDOW` immediately after an incomplete edit; the process exited and the newest incomplete draft returned exactly in the next writer session |

The earlier process termination test and normal-close test are separate. The
former checks an already persisted draft; the latter checks close immediately
after native text input. Source comparisons include the final newline and the
unfinished suffix. Workspace hash checks read the actual file, not a UI status
label. Revision isolation compares full saved objects, not just revision numbers.

## Screenshot evidence

- [Invalid code and protected board](p1-native/invalid-code.png)
- [Second instance read-only](p1-native/second-instance.png)
- [Minimum window with passing simulation](p1-native/minimum-window.png)
- [Published revision and preserved revision-1 instances](p1-native/published-revision.png)
- [Passing live HTTP run](p1-native/live-result.png)
- [External-change conflict](p1-native/external-conflict.png)
- [Reload with an explicitly stale draft](p1-native/reloaded-stale-draft.png)
- [Newest incomplete draft after normal close/restart](p1-native/close-restart.png)

## Repeating the pass

Use a fresh Xvfb display and fresh data/config directories. Launch the debug
binary normally, rather than with `--project` (which disables durable workspace
storage). Use Ctrl+E, select all source, then paste the default source with the
scenario rename/probe above. Repeat the invalid suffix, undo/redo, close/restart
and second-instance steps. Use the inspector to rename the probe, then open a
component and publish a changed revision. Start the local demo server before the
successful API run. For external-change testing, edit only the scratch
`data/apiforge/workspace.json`, then make a GUI edit to trigger the checked save.
Compare file hashes and parsed JSON before choosing Reload.

## Limits and remaining stage work

No new functional issue was reproduced in these paths. This is a bounded native
regression pass, not exhaustive product acceptance. File-portal dialogs, drag and
drop, assistive technology, physical power loss and Windows/Wayland-native input
were not tested. No assertion is made that the transient journal creation was
visually observed; only the successful result and final absence were verified.

The native app still uses the legacy JSON workspace. Folder storage APIs from P1
are covered by the [P1 fault/process/cache tests](../../stage-ii-p1-acceptance.md),
not invoked through this GUI. Folder loading, independent document sessions,
folder CLI commands, explorer, migration and startup activation remain their
planned stages. Windows directory durability remains a P7 gate. This QA pass does
not close the P2 semantic/identity/layout round-trip gate.
