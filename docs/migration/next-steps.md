# Remaining native application work

Updated 23 October 2026 after integrating the five Argon branches and correcting
review findings. Work continues directly in this checkout with the standalone
[Gemini/KWin workflow](../../AGENT_WORKFLOW.md).

See [the code review](argon-code-review.md) for findings, fixes and exact validation.
The [original QA audit](../../QA_AUDIT_TASKS.md) retains reproduction steps; it is
not a statement that every issue remains open or that every proposed fix shipped.

## Current priorities

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

The latest UI follow-up passes 241 workspace tests, formatting, Clippy with
warnings as errors, compatibility fixture verification and a workspace debug
build. Screenshots and direct KWin interaction evidence, including the incomplete
Gemini attempt after its account quota was exhausted, are documented in the
[UI validation notes](ui-polish.md). The repository remains local; no push is part
of this workflow.

The earlier JSON follow-up's focused Gemini GUI regression passed, including
report keyboard resizing, duplicate contracts, the 960×540 modal and JSON tree
interactions. Exact scope and evidence are in [the review](argon-code-review.md).
