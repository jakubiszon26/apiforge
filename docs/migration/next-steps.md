# Remaining native application work

Updated 23 October 2026 after integrating the five Argon branches and correcting
review findings. Work continues directly in this checkout with the standalone
[Gemini/KWin workflow](../../AGENT_WORKFLOW.md).

See [the code review](argon-code-review.md) for findings, fixes and exact validation.
The [original QA audit](../../QA_AUDIT_TASKS.md) retains reproduction steps; it is
not a statement that every issue remains open or that every proposed fix shipped.

## Current priorities

The active feature work is the [scenario code and canvas plan](../plans/code-and-canvas.md).
Its milestone order takes storage safety (item 4, STOR-01) as milestone S1 directly
after the format change (M1), before anything else extends what is stored. Items
5 and 6 and Windows evidence stay release gates and are scheduled between the
plan's milestones. UI polish (item 1) continues only where a milestone touches the
screen.

1. **UI polish:** the header now combines title, project context and actions;
   typography, spacing, card zoom and symbolic action buttons have been revised.
   Tabs and toolbars follow the requested Postman-inspired direction. See the
   [layout and validation notes](ui-polish.md). Continue checking real screens
   and user feedback before moving to lower-priority feature work.
2. **JSON performance (PERF-01):** picker rows are now virtualised and inspector
   branches are lazy, paginated by 100 fields and bounded to 400 rows per rendered
   output tree. The core collector preserves its 1000-field/depth-12 cap and now
   reports real truncation explicitly in both pickers. Remaining follow-ups are
   picker caching and pagination/budgeting in the report's separate JSON view.
3. **Built-in demo API (FUNC-07):** review the preserved unfinished implementation
   in `.gui-qa/history/unfinished-M01.patch`, especially shutdown blocking, before
   reuse. The desktop still needs a separate demo API executable for Live mode.
4. **Storage (STOR-01):** prevent concurrent writers, detect external changes and
   provide deliberate backup recovery. Existing atomic writes do not cover these.
5. **Interrupted Live run journal (RUN-02):** record uncertain effects after a crash.
   Distinguish DNS/TLS failures beyond the current broad reqwest prefixes.
6. **Accessibility (A11Y-01):** verify that AccessKit labels reach AT-SPI and test
   actual keyboard/screen-reader use. Labels alone are not acceptance evidence.

Board overlay layout, section alignment, contract controls, the searchable
variable-response picker, condition-path drops, keyboard report resizing,
Delete routing, minimap navigation and compact layout are implemented, including
review corrections. The logical window minimum is now 960×540. Test this client
size explicitly; 800×600 is still below the supported minimum width.

`claude-backlog.json` retains older implementation detail as reference, not an
executable controller plan. Windows execution, release packaging/signing and
assistive-technology acceptance still require independent evidence.

## Local follow-up validation

The format-v2 follow-up on `feat/format-v2` now protects version-one workspaces
from backup fallback and ordinary saves. Startup archives the original and any
backup without replacing an earlier archive, then opens a fresh demo workspace;
an archive failure disables autosave. A regression test failed on the original
fallback behaviour before the fix. All 258 workspace tests, formatting, Clippy
with warnings as errors, both fixture checks and the workspace debug build pass.
The six new tests cover storage protection, archive conflicts, exact preservation
and desktop startup/save behaviour. Native Windows execution and the remaining
M1 product acceptance still need evidence; S1 follows M1.

The latest UI follow-up passes 241 workspace tests, formatting, Clippy with
warnings as errors, compatibility fixture verification and a workspace debug
build. Screenshots and direct KWin interaction evidence, including the incomplete
Gemini attempt after its account quota was exhausted, are documented in the
[UI validation notes](ui-polish.md). The repository remains local; no push is part
of this workflow.

The earlier JSON follow-up's focused Gemini GUI regression passed, including
report keyboard resizing, duplicate contracts, the 960×540 modal and JSON tree
interactions. Exact scope and evidence are in [the review](argon-code-review.md).
