# AB-01 round 4: the task sheet

Ticket: [AB-01](../../../../tickets/AB-01.md) (*Rounds 2 to 4*) · issue
#537 · method: [A/B rounds](../../../../process/ab-testing.md). Run on
2026-11-23 by an Opus coordinator; every run driven by a fresh Opus
subagent. Rounds 2 and 3 are the model; their lessons are applied (see
*Where the round departed from the runbook*). The last round.

## The variants

| | Build | Start |
| --- | --- | --- |
| A | `apiforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` of the GitHub release, `SHA256SUMS.txt` verified | T6: a copy of `crates/apiforge-cli/tests/fixtures/rooms-language-1` (language 1); T7: the cart scenario below in language 1 |
| B | `apiforge-v0.2.1-rc.2-x86_64-unknown-linux-gnu.tar.gz`, the same | T6: a copy of `fixtures/scenarios/lay/rooms` (language 2); T7: the cart scenario below in language 2 |

The tarballs are round 3's downloads, checked again against the
`SHA256SUMS.txt` downloaded afresh from each release, and unpacked
afresh for this round.

- **T6**: the fixtures unchanged.
- **T7**: a project *Shop* (base address `http://127.0.0.1:4318/demo`)
  holding one scenario, `scenarios/cart.afs` (*Cart round trip*),
  written by the coordinator once per language, in each version's own
  format (`fmt --check` exits 0), with `123` and `2` once each and no
  samples:

  ```
  scenario "Cart round trip" {
    description "Creates a cart, reads it back by its id, and deletes it."
    begin = start
    @after(begin)
    created = POST "/shop/carts" {
      body { productId: 123, quantity: 2 }
    }
    cart = GET "/shop/carts/{{created.body.id}}"
    same_cart = assert cart.body.id == created.body.id
    same_quantity = assert cart.body.quantity == created.body.quantity
    cleanup {
      deleted = DELETE "/shop/carts/{{created.body.id}}"
      delete_worked = assert deleted.status == 200
    }
  }
  ```

  Language 2 is the same without the `@after(begin)` line. Before the
  first counted run each passed round 1's T2 check with the demo API
  built from `main` and started for it: `check` exits 0, the live run
  passes twice with nothing changed, the `POST` sends `productId` 123 and
  `quantity` 2, the `GET` and the `DELETE` read the `POST`'s `id`, two
  checks read the `GET`'s response, the `DELETE` is in `cleanup`. The
  demo API was stopped by its process id before the first run. Without
  samples the simulated run stops at `created`; T7 does not ask for a
  run.

Every run starts from a fresh copy. No server during the runs: every run
and every check is a simulation. The fixtures in the repository were
not touched.

## Before the first counted run

- Each of the four prepared projects was opened in its version's window
  (`kwin`, isolated home, 1280 × 800). v0.1.0 opens both on *Code*: T6 on
  `booking-day.afs`, its lines 1 to 39 on the first screen (the
  sign-in, `profile`, `team`, `big_enough`, `buildings`, `floors`,
  `rooms`, `equipment` and the start of `policy`, with their `@after`
  lines); T7 on `cart.afs`, the whole file on the first screen.
  v0.2.1-rc.2 opens both on *Diagram* with the *Library*. Opening wrote
  only `.apiforge/diagram.json` and `.apiforge/local/`.
- Each version's `check --folder` exits 0 on each opened copy
  (v0.2.1-rc.2 with the warning "A far step reads this value directly;
  save it as a variable"). Its `run --json` passes on the T6 copy in
  simulation; on the T7 copy it stops at `created` for want of a sample,
  as above.
- [check.sh](check.sh) was tried per version on hand-made results:
  - T6 passing: the new `GET` written after `profile`, `policy` and
    `short_enough` moved above `equipment` (language 1 with
    `@after(profile)` on the new step, `@after(photos)` on `team`,
    `@after(has_floors)` on `policy`, `@after(short_enough)` on
    `equipment`);
  - T6 failing: the unchanged start; the new `GET` after the team; a
    check of the start changed; language 1 with `policy` before
    `equipment` only by a tie between siblings (no `@after` between
    them); language 1 with the new `GET` placed after `big_enough`;
  - T7 passing: `quantity` 3 and `productId` 124; failing: only the
    quantity changed, and both values changed with the delete check
    changed too.

  Every passing case passed and every failing case failed, on both
  versions. One case made to fail on language 2 passed: `policy` moved
  above `equipment` and its check `short_enough` written below
  `has_screen`. Language 2 runs a check right after the request it
  reads, so in the run `short_enough` still comes before `equipment`;
  by the sheet's key (the order of the run) it passes, and it is not a
  failing case.

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
> The task: ‹T6 or T7›
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
> lines so far). A log rebuilt afterwards makes the run void. For each
> `WRONG TURN` line, add the path of the screenshot that shows it (the
> path `screenshot` returned); those screenshots are kept.
>
> Process rules: you may not use a terminal at all — no Bash or shell
> tool of any kind, not even `echo` or a command that prints nothing; one
> terminal call makes the run void. Never `pkill -f` or `pgrep -f`, and no
> loop that waits on a process list; stop only what you started, with
> `session_stop`. Before you return, make sure you have called
> `session_stop`.
>
> In your final report, besides the log and the sentence, say in one line
> what the window showed when it first opened and whether you used
> anything from that first screen.

(`$R` was the literal scratch path.) For T7 the brief was changed as the
runbook says: *open the Code view or the Split view* left out of what
the driver may not do; after the task, "Take a screenshot each time you
see the new value in the other view", with the two paths written in the
log; and the project named *Shop* in the session start. See
*Where the round departed* for the API paragraph. For a shortest path
the list of actions was appended, with: "look at the window before
each; if one does not do what it says, go on as a tester would and log
it".

**T6.** "In the test *Room booking day*: add a request `GET
/rooms/room-a/photos` that is sent after the profile is read and before
the team is read. Then make the booking policy be read, and checked,
before the room's equipment. Run the test until it passes."

**T7.** "Open the test's code and change the quantity from 2 to 3 there;
look at the diagram and see 3. Then, on the diagram, change the product
from 123 to 124; look at the code and see 124. Save."

## The pass check

[check.sh](check.sh) `T6|T7 <cli> <project> <start>`, with the CLI of the
run's own version. It finds the scenario by its registered path, runs
`check --folder` and `run --scenario <path> --json` (simulation) on the
result and on a scratch copy of the start, and compares the two
snapshots with references read by step name and value steps followed:

| | Passes when |
| --- | --- |
| T6 | `check` exits 0 and the simulated run passes; one new `GET` whose sent address ends in `/rooms/room-a/photos`, which in the run's order is after `profile` and before `team`; `policy` and `short_enough` before `equipment` in the run's order; no step or check of the start changed or gone, order edges apart; in language 1, a path of order edges and value reads from `profile` to the new step, from it to `team`, and from `policy` and `short_enough` to `equipment` (a tie between siblings does not count) |
| T7 | `check` exits 0; the `POST`'s body reads `quantity` 3 and `productId` 124, both literals; no other step or check changed, added or gone. The two captures, read by the coordinator, show 3 on the diagram and 124 in the code |

It prints the order of the run, the order edges added and gone, and the
file's diff from the start, from which *The diff* is counted.

## Where the round departed from the runbook

- **The budget decided the order.** The first run, T6 on v0.1.0, took
  40.6 of the round's 90 minutes of agents. To have the shortest paths
  of both versions, as round 3 learned, the round ran the first
  repetition (runs 1 of both tasks on both versions), then the shortest
  paths, then the second runs of T7 that still fitted. Runs 2 and 3 of
  T6 and run 3 of T7 were not made, nor the shortest path of T6 on
  v0.1.0: its only run is 122 lines long and could not be replayed in
  what was left. See [results](results.md#what-was-not-run).
- **The shortest paths come from the only run of each cell**, with its
  wrong turns, the actions that undid them, hovers and `DRIVER` lines
  taken out; with one run there was no clean run to choose.
- **The API paragraph of T7.** The project of T7 is a shop, not the room
  booking, so for T7 the paragraph read "the project tests a shop API
  that is not running"; one word, the same on both versions.
- **The terminal.** Every brief forbade a terminal outright, "not even
  `echo`" (round 3 lost a run to one). No driver used one.
- **The first screen.** v0.1.0 opens T6 on its source, which shows the
  `@after` lines; the brief was kept word for word and each driver says
  what it saw first and whether it used it (see
  [results](results.md#read-this-first-the-first-screen)).
- **Captures**, as rounds 2 and 3: `keep_screenshots: true`, the paths in
  the log, copied by the coordinator.
- **A void attempt of the coordinator's making.** The first attempt at
  T7's shortest path on v0.2.1-rc.2 started on a folder the coordinator
  had failed to copy (the window said "Invalid project-relative path.");
  the driver stopped before its first action. It was made again with a
  fresh driver on a fresh copy; its 0.6 minutes are counted in the
  budget.
