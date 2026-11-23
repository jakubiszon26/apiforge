# AB-01 round 2: the task sheet

Ticket: [AB-01](../../../../tickets/AB-01.md) (*Rounds 2 to 4*) · issue
#537 · method: [A/B rounds](../../../../process/ab-testing.md). Run on
2026-11-23 by an Opus coordinator; every run driven by a fresh Opus
subagent.

## The variants

| | Build | Start |
| --- | --- | --- |
| A | `apiforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` of the GitHub release, `SHA256SUMS.txt` verified | a copy of `crates/apiforge-cli/tests/fixtures/rooms-language-1` (language 1) |
| B | `apiforge-v0.2.1-rc.2-x86_64-unknown-linux-gnu.tar.gz`, the same | a copy of `fixtures/scenarios/lay/rooms` (language 2) |

Both copies are changed the same way: in `scenarios/double-booking.afs`,
`second`'s sample reads

```
sample 409 { error: { code: "slot_conflict", message: "The room is already booked for this time." } }
```

Nothing else differs from the fixtures, which were not touched. Every run
starts from a fresh copy; T4 never starts from a T3 result. No server:
every run and every check is a simulation.

## Before the first counted run

- Each prepared project was opened in its version's window (`kwin`,
  isolated home, 1280 × 800). v0.1.0 opens it on *Code* with
  `booking-day.afs`; v0.2.1-rc.2 on *Diagram* with `booking-day.afs` and
  the *Library* showing. Opening wrote only `.apiforge/diagram.json` and
  `.apiforge/local/`.
- Each version's `check --folder` exits 0 on its copy (v0.2.1-rc.2 prints
  warnings "A far step reads this value directly; save it as a variable",
  and exits 0), and its `run --scenario scenarios/double-booking.afs
  --json` passes in simulation.
- [check.sh](check.sh) was tried per version on a hand-made passing and a
  hand-made failing file for each task: T3 passing with
  `rooms.body.items[0].id` and with `first.body.roomId`, failing with the
  literal `"room-a"`; T4 passing with `typeOf(first.body.id) == "string"`
  (both languages), `first.body.id is "string"` (language 2) and a code
  read through a value step, failing with the message compared in place
  of the code, no type check and `refused` changed. Every passing file
  passed and every failing file failed, on both versions.

## The start state of a run

- The window through the `kwin` MCP server: `session_start` with
  `app_command` = `<build>/apiforge-desktop --folder <copy>`,
  `isolate_home: true`, `keep_screenshots: true`, 1280 × 800.
- Neutral paths: the builds at `$R/a` and `$R/b`, the copies at
  `$R/p<number>/proj`, with `$R` a scratch folder outside the repository.
  The driver goes on when the window shows the project.

## The driver's brief, as given

> You are testing a desktop application as a functional tester who has
> never seen it. Do the task below in its window, using only the pointer
> and the keyboard through the `kwin` tools. Start the session exactly as
> given. Look at the window after every action that changes it; never
> send input you have not seen the target of.
>
> You may not: edit or read the project's files outside the window; use
> a terminal, the `apiforge` command or any `apiforge` MCP tool; read the
> application's source or documentation; open the *Code* view or the
> *Split* view. Never run `pkill -f` or `pgrep -f`. If you cannot go on,
> stop and say where.
>
> Keep a log, one line per action: its number; `click`, `drag`, `menu`,
> `type` or `scroll`; what you acted on, by its label; for `type`, the
> exact text; and `WRONG TURN` with what you expected when an action did
> not help and you gave it up. Mark a missed click or a wait too short
> as `DRIVER`. When you believe the task is done, save with the window's
> *Save*, take one screenshot, stop the session, and return the log and
> one sentence: done, or where you stopped.
>
> What a tester knows: the project tests a room-booking API that is not
> running. Tests are run in simulation; do not send live requests.
>
> The task: ‹T3 or T4›
>
> The session start. Call the `kwin` tool `session_start` with
> `app_command` = `$R/‹a or b›/apiforge-desktop --folder
> $R/p‹n›/proj`, `isolate_home: true`, `keep_screenshots: true`,
> `screen_width: 1280`, `screen_height: 800`. Take a screenshot and go on
> only when the window shows the project (*Room booking*); input sent
> before the window has drawn is lost. The `kwin` tools may be deferred:
> load them first with ToolSearch (query `+kwin`, max_results 40). If you
> maximise the window, say so in the log.
>
> The log is written as you go: before each action, write its line to the
> file `$R/logs/p‹n›.log` (rewrite the whole file with the Write tool, all
> lines so far; never a terminal). A log rebuilt afterwards makes the run
> void. For each `WRONG TURN` line, add the path of the screenshot that
> shows it (the path `screenshot` returned); those screenshots are kept.
>
> Process rules: never `pkill -f` or `pgrep -f`, and no loop that waits
> on a process list; stop only what you started, with `session_stop`.

(`$R` was the literal scratch path.) For a shortest path the list of
actions was appended.

**T3.** "In the test *Double booking is refused* the second booking asks
for a different room than the first. Make it ask for the room the first
booking asks for, whichever room that is. Run the test."

**T4.** "In the test *Double booking is refused*, add two checks: that the
refusal of the second booking carries the error code `slot_conflict`,
and that the id of the first booking is text. Run the test until it
passes."

## The pass check

[check.sh](check.sh) `T3|T4 <cli> <project> <start>`, with the CLI of the
run's own version. It finds the scenario by its registered path, runs
`check --folder` and `run --scenario <path> --json` (simulation) on the
result and on the start, and compares the two snapshots with references
read by step name and value steps followed:

| | Passes when |
| --- | --- |
| Both | `check` exits 0; the run is a simulation and passes |
| T3 | `second`'s `roomId` reads `rooms.body.items[0].id` or `first.body.roomId` (or whatever `first`'s `roomId` reads), not a literal; the simulated `second` sends `first`'s room; no step or check is added, removed or changed apart from that `roomId` |
| T4 | an `equals` check between `second.body.error.code` and `"slot_conflict"`, and a check that `first.body.id` is a string (`is "string"`, or `typeOf(…) == "string"`), both run and passed; `refused`, `one_booking`, `not_booked` and `gone` unchanged |

It prints the file's diff from the start, from which *The diff* is
counted.

## Where the round departed from the runbook

- **Captures.** The runbook has the driver copy its wrong-turn screenshots
  to `$R/captures/`, but the brief forbids it a terminal. The session was
  started with `keep_screenshots: true` instead, each `WRONG TURN` line
  names its screenshot, and the coordinator copied them after the run.
- **The log as it goes.** Each driver rewrote `$R/logs/p<n>.log` with its
  file-writing tool before each action; the runbook says when, not how.
- **`check.sh` after run 1.** v0.1.0's window writes `@config` without
  `bodyFormat` when it saves a `POST`, and the first version of the
  script counted that as a changed step. An absent `bodyFormat` now reads
  as its default, `json` (as v0.1.0's own `@config` check does); the
  order edges are printed beside the comparison. The hand-made files were
  tried again with the same results before run 2.
- **The test's title.** The sheets name the test by its title, *Double
  booking is refused*; neither window shows a scenario's title. The
  drivers were told nothing more; every one chose `double-booking.afs`
  by its file name, and the wrong turns spent looking are counted.
- **Keys and hovers.** The brief's verbs have no word for a key or a
  hover; drivers logged keys as `type` or `key` and hovers in words. Keys
  are counted apart, as round 1 did; a hover is counted only when the
  driver marked it a wrong turn.
- **The shortest paths.** Where two runs tie, the one with fewer actions
  in all (T4 on v0.2.1-rc.2) or without a maximised window (T3 on
  v0.2.1-rc.2) was replayed. T3 on v0.1.0 replayed run 1, although run 3
  is one pointer action shorter, because run 3's edit rests on a missed
  click (see [results](results.md#the-shortest-paths)). The lists kept
  every action that was not a wrong turn or its undoing, the zoom clicks
  included.
