# AB-02: the task sheet

Ticket: issue #583 · runbook: [AB-01](../../../../tickets/AB-01.md)
(*Round 1* and *Rounds 2 to 4*) · comparison: [AB-01 round 4](../AB-01/round-4/)
(v0.1.0's recorded T6 run, not run again). Run on 2026-11-23 and
2026-11-24 by Opus coordinators; every run driven by a fresh Opus subagent.

## The variant

| | Build | Start |
| --- | --- | --- |
| main | a release build of `origin/main` at `fe74ae73` (DIAG-06b, #584, merged), `cargo build --release --locked -p apiforge-desktop -p apiforge-cli`, toolchain 1.99.0, its own target folder | a copy of `fixtures/scenarios/lay/rooms` (language 2), unchanged |

The build sits behind a neutral path `$R/b`, the copies at
`$R/p<number>/proj`, `$R` a scratch folder outside the repository. Before
the first run the build's `check --folder` exits 0 on the start (with the
"A far step reads this value directly" warnings) and its `run --scenario
scenarios/booking-day.afs --json` passes in simulation.

## The brief, the task and the pass check

Round 4's, word for word: the brief in
[round 4's task.md](../AB-01/round-4/task.md#the-drivers-brief-as-given)
with `‹b›` and the run's number filled in, and T6:

> "In the test *Room booking day*: add a request `GET
> /rooms/room-a/photos` that is sent after the profile is read and before
> the team is read. Then make the booking policy be read, and checked,
> before the room's equipment. Run the test until it passes."

[check.sh](check.sh) is round 4's script, unchanged apart from its title
line: `check.sh T6 <cli> <project> <start>` with the build's own CLI.

## Where this departs from the runbook

- **The runs span two days.** Run 1 was made on 2026-11-23; the working
  day ended at about 23:35 and the runs that did not fit were not
  started. Runs 2 and 3 and the shortest path were made on 2026-11-24 on
  a fresh release build of the same commit `fe74ae73`, the start verified
  again as above.
- **The shortest path's list** was rebuilt from [results](results.md#the-shortest-path),
  as run 1's log was lost in a restart of the machine.
- **The void attempt** at run 2 is recorded under
  [void runs](results.md#void-runs).
