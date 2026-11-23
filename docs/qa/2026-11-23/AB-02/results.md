# AB-02: results (partial)

T6 of [AB-01](../../../../tickets/AB-01.md) on a release build of `main`
at `fe74ae73` (DIAG-06b, #584), against v0.1.0's recorded run in
[AB-01 round 4](../AB-01/round-4/results.md). The sheet and the pass
check are in [task.md](task.md). Nothing here is classed.

**Partial.** One first-time run of three was made on 2026-11-23 before the
end of the working day. Runs 2 and 3 and the shortest-path replay were
not started. The retest resumes from here.

Run on 2026-11-23, Linux, the `kwin` MCP server at 1280 × 800, isolated
home, simulation only. Each cell reads **shortest path · run 1 · run 2 ·
run 3**; `—` is a run not made. Never an average. Figures leave out the
lines a driver marked `DRIVER`.

## T6 — a new request between profile and team; policy before equipment

| | v0.1.0 (AB-01 round 4) | main after DIAG-06b |
| --- | --- | --- |
| Done | — · yes · — · — | — · yes · — · — |
| Pointer actions | — · 89 · — · — | — · 12 · — · — |
| Text entries | — · 6 · — · — | — · 2 · — · — |
| Typed / syntax | — · 86/0 · — · — | — · 22/0 · — · — |
| Scrolls | — · 14 · — · — | — · 3 · — · — |
| Wrong turns | — · 14 · — · — | — · 0 · — · — |
| Wrong result | none | none |
| The diff | — · 4 (+7) · — · — | — · 2 · — · — |
| Minutes | 40.6 | 2.3 |

For reference, v0.2.1-rc.2 in round 4: shortest path 44 pointer actions,
4 wrong turns; run 1 58 pointer actions, 11 wrong turns, 18.4 minutes.

- **The way, main.** Select `profile`; *Add step* → *HTTP request*. The
  new request joins the chain right after `profile`. Type the address
  over `/endpoint`. Right-click `policy`; **Move earlier** in its menu.
  One move put `policy` and its check `short_enough` before `equipment`.
  *Run* failed with "No sample response for the simulation."; open
  *Contract and sample response*, set the sample body `[]`, *Run*
  (passed, 32 passed, 1 skipped), *Save*.
- **Found:** *Move earlier*, in the step's right-click menu, on the
  driver's first look for a move (log line 6). The `+` on a line and
  Alt+↑/↓ were not used. No refusal message was seen.
- **Typed.** The address `/rooms/room-a/photos` (20) and the sample body
  `[]` (2). No language syntax.
- **The sample response** was again reached at the foot of the side panel:
  two wheel scrolls that did not move the panel (`DRIVER`), one scroll
  that overshot (`DRIVER`), and collapsing *Run results* to give the
  panel room. Not counted as wrong turns. The finding is REQ-15 (#576).
- **`check.sh`: PASS.** Run order `… profile request team … rooms policy
  short_enough equipment has_screen …`; no step or check of the start
  changed or gone. *The diff* beyond the task: a `@config` line and a
  `timeout 10s` line on the new request, as on v0.2.1-rc.2. The new
  request kept the name `request`.
- Window not maximised. The first screen was the *Diagram* of
  `booking-day.afs`, Main, 29 steps; the driver worked from it.

## Main to Cleanup

T6 does not move a step between Main and Cleanup. Neither does any
command of DIAG-06b. *Move earlier* and *Move later* move a step by one
place inside its braces, and are disabled at the first and the last
place (DIAG-06b criterion 4). In the code, the move is applied to the
board it came from (`Command::MoveStep` → `apply_move(…, cleanup, …)` in
`crates/apiforge-desktop/src/folder/diagram.rs`). **The window has no way
to move a step from Main to Cleanup.** Whether copy on Main and paste on
Cleanup works was not tried.

## Void runs

None. The driver used no terminal, did not open *Code* or *Split*, and
called `session_stop`.

## What was not run

Runs 2 and 3 and the shortest-path replay. The working day ended. The
coordinator's attempt to start run 2 was refused by the session's
permission check, and the coordinator stopped. About 5 agent minutes
were used.
