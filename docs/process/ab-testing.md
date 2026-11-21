# A/B rounds: variants compared by agents on a user's task

- State: **in force** since 2026-11-21, approved by the owner with the
  options recorded on #463 and #537: the shortest path and three
  first-time runs per variant; a round only for tickets a sprint's plan
  names; 90 minutes of agents and 15 of the owner per round.
- The owner's words: the REQ sprint's acceptance scenario "is also to be
  run by agents, and the most ergonomic interface worked out by A/B
  tests"; A/B rounds are wanted for other features too; and the **first
  round compares v0.1.0 with v0.2.0** — "what got worse will have to be
  addressed" ([AB-01](../tickets/AB-01.md)).

A [comparison board](planning.md#mock-ups) shows how options look. It does
not show how many steps a task takes, where a person turns the wrong way,
or whether the result is right the second time it runs. An A/B round
answers that: agents do the same task in the real window on each variant,
and what they needed is counted. The owner then chooses with the board and
the numbers side by side.

## What a round is

| Part | Rule |
| --- | --- |
| Question | One sentence the round answers: "which of A and B gets a tester from a response field to a check in fewer steps and fewer mistakes". A round with no question is not run |
| Variants | Two to four, one of them **today** (the released build, or `main`). Named as on the comparison board: *Today (v0.2.0)*, *A · …*, *B · …* |
| Task sheet | What a tester would be told, in one to three sentences, and nothing about how. The start state (project, window size, stored settings: none). The **pass condition**, checked on files and with the CLI, never on the agent's word |
| Driver | An agent at the real window: pointer and keyboard only |
| Record | `docs/qa/<date>/<round>/results.md`, the task sheet beside it, at most four captures |
| Decision | The owner's, in a quiz, on the board and the table. The planning model recommends; no agent chooses |

Three kinds of round use the same parts:

- **Version against version** — two released builds on the same tasks, to
  find what got worse. Each regression becomes a ticket
  ([below](#regressions)).
- **Variants of a planned change** — before a ticket's analysis is
  approved, when its question is about a gesture or a flow and the board
  alone cannot settle it.
- **Baseline** — one variant only: an acceptance task run by an agent
  before the owner runs it, as LAY-12 did. It finds what the owner would
  otherwise find at the window.

## How variants are built

In order of preference:

1. **Released builds**, for a version round: the packages of the GitHub
   releases, checksums verified. Nothing is built.
2. **A variant switch on one throwaway branch** `mock/<round>`: the
   variants live side by side behind `APIFORGE_VARIANT=a|b`, read only in
   a build with the cargo feature `variants`, which no release enables.
   One build serves every variant, so the variants differ in nothing but
   the thing compared. The switch and the losing variants are deleted
   before the ticket's pull request is opened; `main` never holds the
   feature.
3. **One mock branch per variant**, `mock/<round>-a`, `mock/<round>-b`,
   only when the variants cannot share a build. Built one after another
   into the same target directory and the binary copied out.

A variant has to work as far as the task goes — the fields write the
`.afs`, the run runs. A drawing cannot be driven; a question that a
drawing answers stays a comparison board. Variant code is throwaway: it
passes no review and is never merged, and the implementation is written
again from the approved ticket, acceptance tests first.

## How the agent drives the window

- The **release binary**, in an isolated KWin Wayland session with an
  isolated home (the `kwin` MCP server: `session_start` with
  `isolate_home`, then `screenshot`, `mouse_click`, `keyboard_type`),
  at **1280 × 800** as LAY-12 used; a round about layout adds 960 × 540.
  Where that server is absent, `tools/gui/host.sh native` with `egui_mcp`
  ([GUI testing](../gui-testing.md#exploration-with-egui_mcp)) serves, and
  the record says which was used.
- The project is a fresh copy in a temporary folder. Live requests go only
  to `apiforge-demo-api` on loopback, started for the round.
- The driver waits for the window before the first input and looks after
  every action that changes the view; input sent blind is a driver error.
- A process the round starts — the demo API, the application — is stopped
  by the process id kept when it was started. `pkill -f` and `pgrep -f`
  are never used: the pattern matches the shell that runs it.
- The driver may not: edit a project file outside the window, use the CLI
  or the MCP server on the project, read the application's source, or
  open Code unless the task sheet allows it. A run that broke a rule is
  void and repeated.
- The driver is an Opus subagent given the task sheet and these rules. It
  is not told the question, which variant is new or which is expected to
  win. The planning model writes the sheet and reads the logs; it does not
  drive a run that is counted.

## Runs

Per variant and task:

- **One shortest path**, written by the planning model once the variant is
  known and replayed by a driver: the fewest actions that pass. This is
  the floor a practised tester reaches.
- **Three first-time runs**, each by a fresh driver that has seen neither
  the variant nor another run: what a newcomer needs. Variants are run in
  alternating order.

Three is enough to tell a difference of kind — a dead end every driver
walks into, a step that exists in one variant and not in the other — and
not a difference of a few clicks. The table reports the three runs, not an
average, and a difference inside their spread is reported as none. A round
is capped at **90 minutes of agent time and 15 owner minutes**; one that
needs more is split or its question narrowed.

## What is measured

| Measure | How it is counted |
| --- | --- |
| Done | Yes / yes after a wrong result / no. Decided by the pass condition: `apiforge check`, `apiforge run` and the content of the `.afs` |
| Actions | Pointer actions (a click, a drag, a menu choice) and text entries, counted from the driver's tool calls; scrolls separately |
| Typed | Characters typed, and of those the characters of language syntax (`{{create.body.id}}`, an operator, a handle) — what a tester has to know by heart |
| Wrong turns | An action that did not advance the task and had to be undone or abandoned; each with the place and what the driver expected |
| Wrong result | A run that passed and should not have, or a check that fails on the second run with nothing changed |
| Hand editing | Whether Code was opened, and why. For a task that forbids it, a void run |
| The diff | Lines of the `.afs` beyond what the task asked for |
| Driver errors | A missed click, a wait too short. Counted apart and left out of every figure above |

Time is written down and not compared: an agent reads a screen in a
second and aims a pointer slowly, a person the opposite.

## The record

`docs/qa/<date>/<round>/`:

- `task.md` — the sheet as given to the drivers, the start state, the
  pass condition.
- `results.md` — the question; one table, variants as columns and
  measures as rows, the shortest path and the three runs in each cell;
  the wrong turns in a list, each with its place; what the round could
  not tell. One page.
- The comparison board of the variants and at most three captures of
  wrong turns.

Raw logs and screenshots of every step are not committed. The issue of
the ticket gets the board and the table; the owner answers in a quiz, and
the answer is recorded in the ticket file as any decision is.

## Who decides

The owner. The table is evidence for a choice, not the choice: fewer
actions do not make an interface the owner wants, and an agent's ease is
not a person's (below). The planning model puts its recommendation first
and says in a line what the numbers do not show. A driver that meets a
choice the owner has not taken — a default, something to remove, the file
format — stops and reports it; it does not pick a variant for itself.

## Regressions

In a version round, a task that is worse on the newer build is a
regression. Each gets a class:

| Class | Meaning | Goes to |
| --- | --- | --- |
| 1 | The task cannot be finished in the window, or finishes with a wrong result or lost work | The next release; it does not ship without the fix or the owner's waiver |
| 2 | The task is finished at a clear cost: a wrong turn every first-time run, or a step that did not exist before | The next sprint |
| 3 | Slower by a few actions, or worse to look at | The backlog |

The planning model proposes the list with a class for each; **the owner
confirms the list and the classes** before any issue is labelled for a
sprint. What got better is listed too, in one line each, so that a fix
does not undo it.

## What a round cannot tell

- Whether a thing is pleasant, readable at a glance or tiring. That stays
  with the owner's eye and the tester's task.
- Anything that depends on aiming: a small target costs an agent nothing
  measurable and a person a great deal, or the reverse.
- Habit. A driver has none; a tester who knows another client expects its
  places. The task sheet can say "you have used a request client before",
  not make it true.
- A difference of a few actions.

A round therefore never replaces the owner's acceptance of a ticket or of
a sprint; it comes before it.

## Where rounds stand in the lifecycle

Nothing changes in the [lifecycle](README.md#lifecycle-of-a-ticket). A
variants round is part of a ticket's **analysis**: after the comparison
board, before the quiz. A baseline round is part of a sprint's
acceptance ticket. A version round is a ticket of its own. A round is run
only for the tickets a sprint's plan names: the planning model proposes
them with the plan and the owner confirms them with it.
