# AB-01 round 1: the task sheet

Ticket: [AB-01](../../../../tickets/AB-01.md) · issue #537 · method:
[A/B rounds](../../../../process/ab-testing.md). Run on 2026-11-21 by an
Opus coordinator; every run driven by a fresh Opus subagent.

## The variants

| | Build | Start |
| --- | --- | --- |
| A | `apiforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` of the GitHub release, `SHA256SUMS.txt` verified | an empty folder (language 1 once initialised) |
| B | `apiforge-v0.2.0-x86_64-unknown-linux-gnu.tar.gz`, the same | an empty folder (language 2 once initialised) |

Live requests went to `apiforge-demo-api` on `127.0.0.1:4318`, started
fresh for each run and for each check and stopped by its process id.

## The start state

- The window through the `kwin` MCP server (the `egui` server was down):
  `session_start` with `app_command` = `apiforge-desktop --folder <empty
  folder>`, `isolate_home: true`, 1280 × 800. No stored settings.
- The driver went on only when the window showed *Open a folder
  project*; *Initialise project* was its first action and is not
  counted.
- The binary and the project folder had neutral paths (no version in
  them), so the driver could not read the version from the command.

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
> What a tester knows about the API: it is at
> `http://127.0.0.1:4318/demo`. `GET /shop/products/{id}` returns a
> product; product 123 exists. `POST /shop/carts` with a JSON body
> `{ "productId": …, "quantity": … }` creates a cart and returns it with
> its `id`. `GET /shop/carts/{id}` returns a cart. `DELETE
> /shop/carts/{id}` deletes it. Requests are to be really sent, not
> simulated.
>
> The task: ‹T1 or T2›

followed by the session start above (the command, `isolate_home`, the
size, *Open a folder project*, *Initialise project* not counted).

**T1.** "Make a new test that asks the shop for product 123, and run it
against the server until it passes."

**T2.** "Write a test that creates a cart with two of product 123, reads
the cart back by the id the API gave it, checks that it is the same cart
with the same quantity, and deletes it, checking that the delete worked.
Run it twice. Then make the delete happen even when a check fails."

## The pass check

With `A` the CLI of the run's own version and `F` the one `.afs` under
`scenarios/`, and the demo API started fresh for the check:

| | Passes when |
| --- | --- |
| Both | `A check --folder P` exits 0; `A run --folder P --scenario scenarios/F --live --json` reports `passed` |
| T1 | exactly one request, a `GET` whose address ends in `/shop/products/123`; the run's step has status 200 |
| T2 | a `POST` to `/shop/carts` with body `productId` 123 and `quantity` 2; a `GET` and a `DELETE` whose addresses read the `id` of the POST's response and hold no literal id; at least one check that reads the GET's response; the `DELETE` inside `cleanup`; and the run passes a second time with nothing changed |

The conditions were read from the run's `--json` snapshot (the graph and
the cleanup graph the CLI loaded), which both versions print, and the
`.afs` was read beside it. Before the first counted run they were tried
on one hand-written T2 file per version (a copy of the rooms fixture in
each language with one scenario replaced): both versions checked, ran
live twice and passed every condition; a first draft whose delete check
expected 204 failed on both, as the demo API answers 200. Every
condition could be stated for language 1.

## Where the round departed from the runbook

- **`F` for language 1.** v0.1.0 saves a new scenario at the project
  root, not under `scenarios/`, so "the one `.afs` under `scenarios/`"
  cannot be stated for language 1. The check used the one scenario path
  registered in `apiforge.project.json`, on both versions.
- **A body from an *Object* step.** In v0.1.0 run 3 and its shortest path
  the POST's body is wired from an *Object* step, not written in the
  request. The condition "a body with `productId` 123 and `quantity` 2"
  was read from the body the run sent.
- **The demo API was not built.** `target/release/apiforge-demo-api` was
  already built from the same source (no commit has touched
  `crates/apiforge-demo-api` since it was built) and was used as it was.
- **Neutral paths.** The runbook's `run-<v>-<T>-<n>` folders and the
  release folder would have shown the version in the command given to
  the driver; neutral names were used instead.
- **The session start in the brief.** The brief says "Start the session
  exactly as given"; the start of step 3 of *One run* was appended to it
  as that paragraph. For a shortest path the list of actions was appended
  as well.
