# MCP-10: The agent's run status in the header

- Size: S · Owner review: 15 min · Risk: `visible UI`
- State: Backlog (issue #94) · MCP-00 decision 17 = A taken 2026-10-27 ·
  mock-up M4 approved 2026-10-27 · after MCP-08b
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md#live-preview-in-the-open-window)

## Goal

While observing, the person sees what the agent is running and how it ended,
without leaving the window: a status line, not the report.

## What the owner will see

- New: a line under the observer notice — *Agent: running
  scenarios/promotion.afs (simulation)…*, then *Agent: promotion.afs passed
  · 18 steps · 12:58* (or *failed at step 7 · Promotional price* /
  *cancelled*); a live run says *(live)*. The line clears when the agent
  finishes.
- Changed, removed, hidden, migrated, defaults: nothing. The full report
  stays in the agent's answer.

## Mock-up

M4: the line in the running and the passed state at 960 × 540.

## Acceptance criteria

1. The agent's `run` sends `run { path, mode, state: started }` and, at the
   end, `state: passed | failed { step, label } | cancelled`; the window shows
   the line within a second of each message.
2. The line shows the mode (*simulation* / *live*).
3. The line is inside the window at 960 × 540 in Code, Diagram and Split and
   does not move the explorer or the tabs.
4. When the channel drops or the agent finishes, the line is gone.
5. The QA report compares the line against M4; the verification commands of
   the development guide pass.

## Not in scope

Showing the agent's report in *Run results*; cancelling the agent's run from
the window; a history of the agent's runs.

## Technical appendix

- The `run` message of MCP-08b's channel, sent by the server's run adapter
  from the executor's start and end; the window keeps `agent_run:
  Option<AgentRun>` in the folder app and renders it under the notice in
  `chrome.rs`.
