# Infrastructure cleanup and initial main-tree review — 23 October 2026

The current dirty checkout at `627aa84` was reviewed without reverting application
changes. Its existing integration fixes cover execution reachability, independent
cleanup timing, shutdown, background file operations, navigation and durable saving.
Formatting, Clippy with warnings denied, all 218 workspace tests, compatibility
fixture verification and a debug workspace build passed on Linux.

## State before integration of the Argon branches

The [20 October audit](../../QA_AUDIT_TASKS.md) describes remaining defects rather
than completed fixes. Source review confirms fixed-position board overlays,
minimum client size 1100×680, no built-in demo API control and a runner completion
flag set only on normal return. Storage still lacks a single-writer lock. The
audit remains the application backlog; passing automated tests does not close it.

The abandoned M01 campaign had not been accepted or applied to the main checkout.
Its demo API implementation and unfinished layout change were exported before
cleanup to `.gui-qa/history/unfinished-M01.patch`. Other private working-tree
diffs, historical plans, reports and GUI evidence were retained under
`.gui-qa/history/`. These patches need review before reuse.

## Removed infrastructure

Removed `tools/agent-env/` and `.agent-environment/`, including the development
controller, dashboards, Gas Town/Beads/Dolt executables and state, private campaign
checkouts and build caches (approximately 59 GiB). No laboratory tmux server was
running when checked. The installation's `~/.gt/hooks-base.json` and
`~/.gt/cmd-usage.jsonl` were also removed. Normal agent authentication and existing
application edits were preserved.

The KWin Python environment and isolated Gemini home were relocated to `.gui-qa/`.
The standalone scripts live in `tools/gui-qa/`. Gemini's MCP configuration now
points directly to `tools/gui-qa/kwin-mcp`; `agy mcp list` confirms it is enabled.
There is no dependency on the removed directory or task controller.

## Actual Gemini verification

`gemini-4-argon-high` completed a real GUI smoke test through the standalone runner:

- Started an isolated KWin Wayland session at 1920×1080.
- Launched API Forge and verified its real window (client 1600×1000).
- Used Ctrl+K, typed `Product`, pressed Enter and inspected the selected block.
- Switched Inspector tabs using pointer clicks, then restored Configuration.
- Captured eight PNGs and stopped the session cleanly.

The report returned **PASS**. The MCP audit contains 15 actual tool calls, zero
MCP errors and a successful `session_stop`. Evidence is stored locally at
`.gui-qa/runs/20261023T151414Z-0ab6075b/` (`gemini-result.json`,
`mcp-audit.jsonl`, and `tmp/kwin-mcp-screenshots-5_k0gg6n/`).

This is acceptance of the standalone testing path and the narrow navigation
scenario. It does not close the earlier visual or accessibility findings. In
particular, the empty AT-SPI application tree remains an accessibility gap even
though the smoke report describes it as expected for a GPU-rendered UI.

Both standalone staging tests pass, including the busy-port check that aborts
before staging or starting a service. Python compilation, shell syntax and
`git diff --check` also pass. See [the workflow](../../AGENT_WORKFLOW.md) for use.

## Subsequent Argon branch review

The later integration review found five `argon_*` branches whose commits were not
present in the working tree described above. Their implementation and corrections
are documented in [the Argon code review](argon-code-review.md). The earlier
218-test result and smoke capture describe the pre-integration build.
