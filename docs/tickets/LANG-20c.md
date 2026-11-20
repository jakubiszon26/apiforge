# LANG-20c: The MCP tool `fix`, and a change of the record alone reaches an open window

- Size: S · Owner review: 10 min (S because LANG-20b returns the whole
  result of a run and LANG-20a the observation; it designs neither) · Risk: `format` (it writes what
  LANG-20b writes, through the same function)
- State: **Decided; labelled when the independent review of LANG-20a and
  LANG-20b is ruled.** Split from LANG-20 on 2026-11-20. After LANG-20b
  (#497). LANG-00, section 3: the window, the command line and the MCP
  server do the same thing because they call the same function.
- Issue [#498](https://github.com/jakubiszon26/apiforge/issues/498) · LANG
  sprint, first half, a condition of 0.2.0 · branch `feat/lang-20c-mcp-fix`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

An agent updates a project the way the command does, and a person
watching in the window sees it happen.

## What the owner will see

- New: the MCP tool `fix` — `check: true` returns LANG-20b's plan as the
  report and writes nothing; without it, the plan is committed;
  `as_written` lists sources converted by hand. It takes the project for
  the agent session as `write_source` does; a window with unsaved work
  refuses.
- New: while the window observes, each of the agent's steps shows: the
  converted sources' text, the count of sources done, which sources can
  be edited on the board — also when the step changed only the record.
- Removed or hidden, migrated: nothing.

## Mock-up

Not applicable: no new element; the server's instructions and the tool's
description gain the tool.

## Acceptance criteria

Tests in `crates/apiforge-mcp/tests/acceptance.rs` (M) and
`crates/apiforge-desktop/src/folder/acceptance_tests/lang20c.rs` (D, with
the MCP harness of `mcp08b`).

1. `fix` gives the same files, record, manifest and report as the
   command on the same project, for the convertible project, the one
   with blocked sources and the one that only needs its finish; with
   `check: true` nothing is written. Test: M `lang20c_criterion_1_*`.
2. A project held by a window with unsaved work, by another agent or by
   a command is refused as `write_source` is; a stale plan is refused
   with LANG-20b's sentence. Test: M `lang20c_criterion_2_*`.
3. A record-only step — `as_written` for one source while others stay
   blocked, so that no source and no manifest byte changes — reaches the
   observing window: the count rises, that source's errors carry their
   fixes, and it can be edited on the board after the agent has finished
   editing. Test: D `lang20c_criterion_3`.
4. When the agent's `fix` finishes the project, the window shows it in
   language 2 and a run starts. Test: D `lang20c_criterion_4`.
5. The tool's result is LANG-20b's result of a run: the five kinds of
   source, the sources converted, `finished`, and — when the run stopped
   — why and at which file. A run that converted some sources and then
   stopped is a **result** (`finished: false` with the converted list),
   not an error: the tool's error (`wrote: false` or `unknown`,
   `apiforge-mcp/src/error.rs`) is kept for a run that wrote nothing or
   whose commit's outcome is not known. The server's instructions name
   the tool and say so. Tests: M `lang20c_criterion_5_*`: a run stopped
   after its first commit; one that wrote nothing; the schema.

## Regression potential

The hand-over and holder rules (MCP-07, MCP-08b: suites unchanged), the
output schemas of the other tools (MCP-14's test), the window following
an agent's commits (MCP-08a suites unchanged).

## Not in scope

The plan and the commit (LANG-20b). The window's own offer (LANG-21a).

## Technical appendix

`apiforge-mcp/src/{tools,server,agent}.rs`; nothing in
`apiforge-project` beyond calling LANG-20b's functions.
