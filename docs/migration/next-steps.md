# Remaining native application work

Updated 23 October 2026 after integrating the five Argon branches and correcting
review findings. Work continues directly in this checkout with the standalone
[Gemini/KWin workflow](../../AGENT_WORKFLOW.md).

See [the code review](argon-code-review.md) for findings, fixes and exact validation.
The [original QA audit](../../QA_AUDIT_TASKS.md) retains reproduction steps; it is
not a statement that every issue remains open or that every proposed fix shipped.

## Current priorities

The active feature work is the [scenario code and canvas plan](../plans/code-and-canvas.md).
M1 (format v2) and S1 (storage safety) are implemented and locally verified on
Linux. The next feature milestone is M4, scenario language and CLI; M2 layout/routing
can follow independently. Its milestone order puts storage safety before anything
else extends what is stored. Items
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
4. **Storage (STOR-01 / S1):** directory-level single-writer sessions, SHA-256
   external-change checks, read-only secondary windows, deliberate backup recovery
   and the draft storage API are implemented and verified on Linux. See
   [ADR 0008](../decisions/0008-storage-sessions.md). Native Windows execution
   remains a release gate. The next feature milestone is M4, the scenario language.
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

The final M1 Export follow-up passes all 281 workspace tests, formatting, Clippy,
both fixture checks and the workspace build. Native KWin editing and revision-save
evidence, the pre-fix regression failure and the new nested/conditional/error Export
tests are recorded in [M1 acceptance](format-v2-acceptance.md).

S1 passes all 273 workspace tests, formatting, Clippy with warnings as errors,
both fixture checks and a workspace debug build. One ignored helper test is
launched and killed by its parent process-lock test; it is not an untested case.
Native KWin evidence covers secondary-window read-only state, an external-change
conflict and reload, and explicit backup recovery at 1600×1000 and 960×540 client
sizes. Details and local screenshots are listed in
[ADR 0008](../decisions/0008-storage-sessions.md).

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
