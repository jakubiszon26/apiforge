# AB-02: results

T6 of [AB-01](../../../../tickets/AB-01.md) on a release build of `main`
at `fe74ae73` (DIAG-06b, #584), against v0.1.0's recorded run in
[AB-01 round 4](../AB-01/round-4/results.md). The sheet and the pass
check are in [task.md](task.md). Nothing here is classed; the planning
model classes it.

Run 1 on 2026-11-23, runs 2 and 3 and the shortest path on 2026-11-24, all
on the same commit; Linux, the `kwin` MCP server at 1280 × 800, isolated
home, simulation only. Each cell reads **shortest path · run 1 · run 2 ·
run 3**; `—` is a run not made. Never an average. Figures leave out the
lines a driver marked `DRIVER`. One attempt at run 2 is void and repeated
([below](#void-runs)); its figures are not in the table.

## T6 — a new request between profile and team; policy before equipment

| | v0.1.0 (AB-01 round 4) | main after DIAG-06b |
| --- | --- | --- |
| Done | — · yes · — · — | yes · yes · yes · yes |
| Pointer actions | — · 89 · — · — | 12 · 12 · 16 · 13 |
| Text entries | — · 6 · — · — | 2 · 2 · 2 · 2 |
| Typed / syntax | — · 86/0 · — · — | 22/0 · 22/0 · 33/0 · 22/0 |
| Scrolls | — · 14 · — · — | 2 · 3 · 2 · 4 |
| Wrong turns | — · 14 · — · — | 0 · 0 · 2 · 1 |
| Wrong result | none | none |
| The diff | — · 4 (+7) · — · — | 2 · 2 · 2 · 2 |
| Minutes | — · 40.6 · — · — | 2.2 · 2.3 · 3.1 · 2.7 |

For reference, v0.2.1-rc.2 in round 4: shortest path 44 pointer actions,
4 wrong turns; run 1 58 pointer actions, 11 wrong turns, 18.4 minutes.

- **The new request.** Run 1 and the shortest path: select `profile`,
  *Add step* → *HTTP request*; it joins the chain right after `profile`.
  Runs 2 and 3: the `+` on the line `profile` → `team`, *HTTP request*.
  Every run typed the address over `/endpoint`; the request kept the name
  `request`.
- **The move.** Every run: right-click `policy`, **Move earlier**; one
  move put `policy` and its check `short_enough` before `equipment`. It
  was the driver's first look for a move in all three runs (log line 6 in
  run 1, 9 in run 2, 7 in run 3). Alt+↑/↓ was not used and no refusal
  message was seen.
- **The sample response.** *Run* failed with "No sample response for the
  simulation."; then *Contract and sample response* at the foot of the
  side panel, a sample body (`[]` in run 1, run 3 and the shortest path,
  `{"items": []}` in run 2), *Run* (32 passed, 1 skipped), *Save*. Each
  collapsed *Run results* to give the panel room. The finding is REQ-15
  (#576).
- **Typed.** The address (20) and the sample body (2; 13 in run 2). No
  language syntax. Keys pressed apart from text: shortest path 0, run 2 1
  (Ctrl+Z), run 3 2 (Escape, Tab).
- **`check.sh`: PASS on each.** Run order `… profile request team … rooms
  policy short_enough equipment has_screen …`; no step or check of the
  start changed or gone. *The diff* beyond the task: a `@config` line and
  a `timeout 10s` line on the new request, as on v0.2.1-rc.2. The
  shortest path and run 3 left byte-identical files.
- Maximised: run 2 (the window was 836 px tall); not in run 1, run 3 and
  the shortest path. The first screen was the *Diagram* of
  `booking-day.afs`, Main, 29 steps; every driver worked from it.

## The shortest path

Run 1 is the shortest passing first-time run and has no wrong turn.
Its log did not survive the night (a restart of the machine), so the
list of 16 actions was rebuilt from its figures and *The way* as recorded
here the same evening: 12 pointer, 2 text and 2 of its 3 scrolls, the
number of wheel inputs left open. Replayed by a fresh driver with every
action doing what the list said and none added. The two scroll lines took
six counted wheel inputs (one more was `DRIVER`), four of them sent
without a log line of their own.

## Wrong turns

All on main. Run 1 and the shortest path have none.

- **Run 2, line 3 (placing the request).** Right-click `profile`, *HTTP
  request* under "After: profile / What should happen next?": expected
  right after `profile` as step 5; it became step 30, at the end after
  `calendar`, fed by `profile`'s data. Undone with Ctrl+Z.
  [capture](t6-main-menu-after-profile-adds-at-the-end.png).
- **Run 2, line 12 (the sample body).** The *Output data* tab: expected a
  place to enter the sample response; it only says a request can have one.
  As on both versions in round 4.
- **Run 3, line 1 (placing the request).** Right-click on the dashed line
  `profile` → `team`: expected that line's menu (insert here); the
  general "Add a step to the scenario" opened. Closed with Escape; the
  line then showed its `+`.

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

- **Run 2, first attempt: void.** The driver reported that it sent
  actions in pairs without looking at the window between them (log lines
  5+6, 10+11, 16+17, 24+25; 21+22 and 26+27 were queued before their
  capture was seen), against "look at the window after every action that
  changes it; never send input you have not seen the target of". It
  passed `check.sh`. Repeated once with a fresh driver on a fresh copy;
  the repeat is run 2 above. For the record, the void attempt: 16 pointer
  actions, 2 text entries, 22/0 typed, 8 scrolls, 3 wrong turns, 3.8
  minutes; *Move earlier* found the same way (line 13); wrong turns: the
  "After: profile" menu (line 2), the right-click on the line (line 4)
  and *Output data* (line 15), the same three places as above.
- No other run is void. No driver reported a terminal or the *Code* or
  *Split* view, and every one called `session_stop`. In run 3 one line
  was written ahead for a click then not made, and corrected at once;
  its line 6 holds the address and a Tab; hovers are not numbered. The
  run is counted.
