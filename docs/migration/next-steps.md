# Remaining native application work

Updated 23 October 2026 after integrating the five Argon branches and correcting
review findings. Work continues directly in this checkout with the standalone
[Gemini/KWin workflow](../../AGENT_WORKFLOW.md).

See [the code review](argon-code-review.md) for findings, fixes and exact validation.
The [original QA audit](../../QA_AUDIT_TASKS.md) retains reproduction steps; it is
not a statement that every issue remains open or that every proposed fix shipped.

## Current priorities

1. **JSON performance (PERF-01):** picker rows are now virtualised and inspector
   branches are lazy, paginated by 100 fields and bounded to 400 rows per rendered
   output tree. The core collector preserves its 1000-field/depth-12 cap and now
   reports real truncation explicitly in both pickers. Remaining follow-ups are
   picker caching and pagination/budgeting in the report's separate JSON view.
2. **Built-in demo API (FUNC-07):** review the preserved unfinished implementation
   in `.gui-qa/history/unfinished-M01.patch`, especially shutdown blocking, before
   reuse. The desktop still needs a separate demo API executable for Live mode.
3. **Storage (STOR-01):** prevent concurrent writers, detect external changes and
   provide deliberate backup recovery. Existing atomic writes do not cover these.
4. **Interrupted Live run journal (RUN-02):** record uncertain effects after a crash.
   Distinguish DNS/TLS failures beyond the current broad reqwest prefixes.
5. **Accessibility (A11Y-01):** verify that AccessKit labels reach AT-SPI and test
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

The JSON follow-up passes 236 workspace tests, formatting, Clippy with warnings
as errors, compatibility fixture verification and a workspace debug build. The
repository remains local; no push is part of this workflow.

Focused Gemini GUI regression also passed on the corrected local build, including
report keyboard resizing, duplicate contracts, the 960×540 modal and JSON tree
interactions. Exact scope and evidence are in [the review](argon-code-review.md).
