# AB-01 round 3: the task sheet

Ticket: [AB-01](../../../../tickets/AB-01.md) (*Rounds 2 to 4*) · issue
#537 · method: [A/B rounds](../../../../process/ab-testing.md). Run on
2026-11-23 by an Opus coordinator; every run driven by a fresh Opus
subagent. Round 2's record is the model; its lessons are applied (see
*Where the round departed from the runbook*).

## The variants

| | Build | Start |
| --- | --- | --- |
| A | `apiforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` of the GitHub release, `SHA256SUMS.txt` verified | a copy of `crates/apiforge-cli/tests/fixtures/rooms-language-1` (language 1) |
| B | `apiforge-v0.2.1-rc.2-x86_64-unknown-linux-gnu.tar.gz`, the same | a copy of `fixtures/scenarios/lay/rooms` (language 2) |

The tarballs are round 2's downloads, their checksums verified again and
unpacked afresh for this round.

- **T5**: in `scenarios/booking-day.afs` (*Room booking day*), `invite`'s
  sample reads `sample 201 { id: "i-1", invited: 4 }` in both copies
  (the fixtures read 5). Nothing else differs.
- **T8**: the fixtures unchanged.

Every run starts from a fresh copy. No server: every run and every check
is a simulation. The fixtures in the repository were not touched.

## Before the first counted run

- Each of the four prepared projects (T5 and T8, per version) was opened
  in its version's window (`kwin`, isolated home, 1280 × 800). v0.1.0
  opens on *Code* with `booking-day.afs`, its source on the first
  screen; v0.2.1-rc.2 on *Diagram* with `booking-day.afs` and the
  *Library*. Opening wrote only `.apiforge/diagram.json` and
  `.apiforge/local/`.
- Each version's `check --folder` exits 0 on each copy (v0.2.1-rc.2 with
  the warnings "A far step reads this value directly; save it as a
  variable"). Its `run --scenario scenarios/booking-day.afs --json`
  fails on the T5 copy at `invited` (4 against 5) and passes on the T8
  copy, both in simulation.
- [check.sh](check.sh) was tried per version on hand-made results: T5
  passing (the window's state added, the answer naming `/invitations`, 4
  and 5), failing with a wrong answer (the second booking, 201 against
  409) and failing with the right answer on a copy whose sample was put
  back to 5; T8 passing with an answer by addresses and (on v0.1.0) one
  by step names, failing with "equal to 5" for the team's check, failing with
  two requests left out and a log line `click Run simulation`, and
  failing with the right answer on a copy whose manifest was changed.
  Every passing case passed and every failing case failed, on both
  versions.

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
> as `DRIVER`. When you have the answer, take one screenshot, stop the
> session, and return the log and the answer. Do not change or save
> anything.
>
> What a tester knows: the project tests a room-booking API that is not
> running. Tests are run in simulation; do not send live requests.
>
> The task: ‹T5 or T8›
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
actions was appended, with: "look at the window before each; if one does
not do what it says, go on as a tester would and log it".

**T5.** "The test *Room booking day* fails. Say which check of which
request fails, what came back and what was expected."

**T8.** "Without running anything: which requests does the test *Room
booking day* send, and in what order? What is checked right after the
request that reads the team?"

## The key and the pass check

[check.sh](check.sh) `T5|T8 <cli> <project> <start> <answer> [<log>]`,
with the CLI of the run's own version. It takes the key from the
simulated run of `booking-day` on a scratch copy of the start, and
checks the result:

| | Passes when |
| --- | --- |
| Both | every file of the start outside the window's own state (`.apiforge/local/`, `.apiforge/diagram.json`) is byte for byte the same, and none is added; `check` exits 0 |
| T5 | the answer names the request to `/invitations` (or its check `invited`), the value 4 that came back and the value 5 expected (the team's size) |
| T8 | the answer names the requests of the key in order, by step name or a word of the address, cleanup's two in either order; it says the team's size is checked to be above 0; the log names no run |

The key, the same on both versions:

- **T5.** The check `invited` (`invite.body.invited == team.body.size`)
  after `POST /invitations`: 4 came back, 5 was expected.
- **T8.** Seventeen requests: the sign-in (one component, `POST
  /sessions`), `GET /me`, `GET /teams/qa`, `GET /buildings`, `GET
  /buildings/hq/floors`, `GET /rooms?floor=2`, `GET
  /rooms/room-a/equipment`, `GET /booking-policy`, `GET
  /calendar/holidays`, `POST /bookings` (`first`), `POST
  /payments/holds`, `POST /invitations`, `POST /notifications`, the
  branch taken: `POST /bookings` (`second`) and `GET
  /rooms/room-a/bookings`; cleanup: `DELETE /payments/holds/h-1` and
  `DELETE /bookings/bk-1`. Right after the team is read, `big_enough`
  checks `team.body.size > 0`.

The script searches the answer for words; the coordinator read every
answer against the key and graded it right, right in part (with what was
missing) or wrong, noting where the grade departs from the script.

## Where the round departed from the runbook

- **Captures**, as round 2: the sessions were started with
  `keep_screenshots: true`, each `WRONG TURN` line names its screenshot,
  and the coordinator copied them after the run.
- **v0.1.0 opens on *Code*.** Its first screen shows the start of the
  scenario's source. The brief was given word for word; each run records
  whether the driver switched to *Diagram* and where its answer came
  from. See [results](results.md#read-this-first-v010-opens-on-the-source).
- **`check.sh` after run 4 and run 12.** The log's test for a run first
  matched the word "run-order"; it now matches *Run*, "ran" and "run the
  test" only. Its test for "above 0" first missed `is greater than" 0`
  with a quote between; it now allows one. The hand-made cases were tried
  again with the same results, and every run was checked again with the
  final script.
- **Grades.** Every answer was read against the key; where the script
  and the grade differ it is said in [results](results.md): the T8
  answers name `second` and `calendar` as the *yes* branch without
  saying it is the one taken (graded right), and the T5 answers that give
  5 without saying it is the team's size (graded right).
- **The shortest paths** were taken from clean runs (no wrong turn, no
  `DRIVER` line), as round 2 learned, even where a run with wrong turns
  is shorter once they are out. The shortest path of T8 on v0.1.0 was
  not run: the round reached its 90 minutes of agents.
- **An interrupted replay.** The first replay of T8 on v0.2.1-rc.2 was
  cut off at its 15th action by a usage limit on the coordinator. Its
  session was stopped with `session_stop` and the replay was made again
  on a fresh copy by a fresh driver.
- **A refused click.** In T8 run 3 on v0.1.0 the driver's click on *Tidy
  up* was refused by the agent session's own permission guard as a
  possible change; the window never received it.
