# MCP-15: A live run from the command line asks the open window — QA report

- Branch: `docs/mcp-15-cli-handover`. Captures are from the release
  binaries of `9549dc1`, the merge of `main` with MCP-10 (#186). All
  scenarios were run again on that build, and every capture was retaken.
  The first captures, of `e9407b6` and `c0f7ec0` (before MCP-10), showed
  the same views. Notes on the code:
  - `c0f7ec0` made the command exit as soon as it has said `released`; see
    Scenario 1.
  - `6c9eadb` moved the reading of the secrets file from `run_live` into
    the CLI. The successful hand-over path is unchanged on screen and in
    the terminal. One failure now comes earlier: a secrets file that cannot
    be read is reported before an unregistered scenario. Either way no
    window is asked.
  - The changes after the first review, below.
  - The merge with MCP-10; see *MCP-10 in the bar*.
  - The waiting line is now *Waiting for the API Forge window to answer*
    (the owner's decision of 2026-11-01). Scenario 2's terminal lines are
    from the `apiforge` binary with that line; the window was the same
    build, and the captures did not change.
- Built with: `cargo build --release --locked` (Rust 1.99.0).
- Compared against the mock-up in this folder:
  (a) `a-command-bar-code-960x540.png` and `a-command-bar-code-1600x1000.png`,
  (b) `b-command-prompt-960x540.png`. The owner approved it as a whole on
  2026-11-01 on #182. It was rendered from `mock/mcp-15-cli-handover` at
  `db71eec`.
- Analysis model: Fable. Implementation model: Claude Opus 5.5.
- Acceptance tests first: commit `362b766`. Before the implementation:
  - 9 of the 10 new CLI tests and the updated one failed. They got *The
    folder project is open in another writer session*, or the holder named
    `run`.
  - Criterion 9 passed, because simulations already behaved correctly.
  - The five desktop tests failed: the command was refused and the window
    never asked.
- The implementation commits changed no assertion. They changed two pieces
  of test setup:
  - The desktop helper passes the secret values, because `run_live` now
    takes them.
  - The CLI's folder snapshot reads the lock file by its length, for
    Windows.

## Criteria and their evidence

| # | Criterion | Tests | Release binary |
| --- | --- | --- | --- |
| 1 | A clean window hands over. The bar shows the command, the badge says *Observing*, no *Take back*. The run sends and journals. The window is the writer again within 2 s, and a save succeeds | CLI `mcp15_criterion_1_…`; desktop `mcp15_criterion_1_a_clean_window_hands_over_and_takes_the_project_back` | Scenario 1 |
| 2 | Unsaved work: the prompt names the command. *Save all and hand over* saves and the run proceeds. *Not now* exits 1 and sends nothing | desktop `mcp15_criterion_2_save_all_…`, `mcp15_criterion_2_not_now_…`; CLI `mcp15_a_window_that_keeps_the_project_sends_nothing` | Scenario 2 |
| 3 | The command waits beyond 30 s and says so once | CLI `mcp15_criterion_3_…` (the window answers after 32 s) | Not beyond 30 s: Scenario 2 shows only the line after 2 s (the answers came at about 10 s) |
| 4 | `permission { live: false }` changes nothing | CLI `mcp15_criterion_4_…` | Scenarios 1–2 (the window's box stayed off) |
| 5 | An agent session is refused by name, and its endpoint hears nothing | CLI `mcp15_criterion_5_…` | Scenario 4 |
| 6 | The other holders are named. The Windows/WSL sentence appears only for an unreachable window on `/mnt/<drive>/` | CLI `mcp15_criterion_6_…`; the updated `a_live_folder_run_requires_its_project_lock_and_a_writable_journal`; unit tests in `apiforge_mcp::command` | — |
| 7 | A passed and a failed run both give the project back within 2 s. A killed command leaves its journal entry, and the window reports it | desktop `mcp15_criterion_7_…` (two tests) | Scenarios 1 and 3 |
| 8 | The window closes after `granted`: exit 0, one stderr line | CLI `mcp15_criterion_8_…` | — |
| 9 | A simulation asks no window, takes no lock and leaves the folder byte for byte | CLI `mcp15_criterion_9_…` | Scenario 5 |
| 10 | The messages in order: `request` (role `command`), `run` running, `run` passed or failed, `released`, EOF | CLI `mcp15_criterion_10_…` | — |
| 11 | Windows: the CLI tests run over the named pipe | the `windows-2022` CI job | see *Windows* |
| 12 | Development guide, comparison against the mock-up, verification | this report | — |

Two further tests:
- `mcp15_the_command_names_itself_run_live_while_it_holds`: the holder file
  says `run --live` and names no endpoint.
- `not_now_for_a_command_leaves_no_agent_wording_in_the_bar`: see *Against
  the mock-up*.

## How the release binaries were driven

The set-up is the same as in the MCP-09 report:
- Linux, Xvfb 1920 × 1200, Mesa's Lavapipe Vulkan driver, the X11 backend,
  and scratch directories for configuration, data and runtime.
- Clicks and keys went through the X test extension: libXtst from the Ubuntu
  archive, unpacked into a scratch directory, not installed. The window was
  given X input focus so that it receives keys.
- Captures were grabbed from the display. The 960 × 540 window is the same
  process, resized through Xlib.
- `apiforge run --folder <shop> --scenario
  scenarios/promotion-and-cart-price.afs --live` ran as a real process. A
  scratch script (not committed) started it and logged its exit code, stdout
  and stderr.
- The project is the demo shop with its base URL set to a scratch loopback
  API. That API answers every request with `200 {"id": 123}` after eight
  seconds, so each run lasts long enough to be seen.

The API's answer lacks the promotion fields, so in these runs the demo
scenario ends *Failed* (exit 1). That verdict is correct for that answer;
see *Not verified*.

## Linux

### Scenario 1 (criteria 1, 4, 7): a clean window, 1600 × 1000 and 960 × 540

1. Open the shop in the release window. The window holds the project: the
   holder file says `role: window` and names its endpoint.
2. Run the command. After three seconds, capture the window and read the
   holder file.
3. Wait for the command to end, reading the holder file at intervals.
4. Repeat at 960 × 540.

Expected:
- The window hands over without asking.
- The bar reads *apiforge run --live · since HH:MM · editing is paused here;
  this window follows every change*. The badge says *Observing*. There is
  no *Take back*.
- stderr says *The open API Forge window handed the project over for this
  run*.
- The journal exists during the run and is gone afterwards.
- The window is the writer again within two seconds of the command's exit.

Observed: as expected.
- The bar during the run, *apiforge run --live · since 22:57 · …*:
  [1600 × 1000](release-bar-code-1600x1000.png),
  [960 × 540](release-bar-code-960x540.png).
- The holder file during the run: `{"role":"command","client":"run
  --live","endpoint":null}`, with `journal.json` present.
- The command ended 8.9 s and 8.6 s after it started; the run made one 8 s `GET`.
  Its stderr held only the hand-over line.
- 0.55 s and 0.60 s after the exit, the holder file was the window's again, with a new
  `since` and the window's endpoint. The journal was gone.
- After the run the window is the writer, with no bar:
  [1600 × 1000](release-after-1600x1000.png).
- *Allow the MCP agent to send live requests* stayed off (the default)
  throughout, and the command still sent (criterion 4).

On the first release run (`e9407b6`), the command left about two seconds
after the window had already taken the project back. Shutting down its
channel runtime waited for work that was already over. `c0f7ec0` drops the
runtime without waiting.

### Scenario 2 (criteria 2, 3): unsaved work, 960 × 540

1. Type into the scenario (line 9: `80 * 1` → `80 * 2`) and into
   `components/product.afs` (`input id: number = 123` → `124`). Collapse
   *components*.
2. Run the command. After three seconds, capture.
3. Click *Not now*; wait for the command to end.
4. Run it again and click *Save all and hand over*. Capture after two
   seconds, then wait for the command to end.

Expected:
- The MCP-08c prompt opens with *A command asks for this project* and
  *apiforge run --live wants to send live requests from this project.
  Unsaved work here:*, and lists both files.
- The command says once that it is waiting.
- *Not now* ends the command with exit 1 and *The window kept the project
  (you chose to keep editing); nothing was sent*. Nothing reaches the API.
  The window stays the writer and keeps the text.
- *Save all and hand over* saves both files, and the run proceeds.

Observed: as expected.
- The prompt: [960 × 540](release-prompt-960x540.png).
- *Not now*: exit 1 after 5.0 s. stderr was *Waiting for the API Forge
  window to answer*, then *The window kept the project (you chose to keep
  editing); nothing was sent*. The API logged
  no request. The window ([capture](release-not-now-960x540.png)) is the
  writer, both files still marked unsaved, no bar.
- *Save all and hand over*: the window showed *Saved 2 files.* and observed
  with the command's bar ([capture](release-saved-observing-960x540.png)).
  stderr had the waiting line and the hand-over line.
- On disk the scenario has `80 * 2` and the component has `= 124`.
- The API logged `GET /products/123`.
- The window was the writer again 0.7 s after the command's exit.

### Scenario 3 (criterion 7): the command killed mid-run, 1600 × 1000

1. Set the scratch API to a 600 s delay.
2. Run the command. After three seconds, while its `GET` is on its way, kill
   it with SIGKILL.

Expected: the journal entry stays. The window takes the project back by its
check and reports the interrupted run, as it does at start.

Observed: as expected.
- Two seconds after the kill, the holder file was the window's, and
  `journal.json` was still there.
- The window showed *A live run did not finish*, with *Promotion and cart
  price*, *Cleanup: NotStarted* and *GET
  http://127.0.0.1:4391/products/123 · Sending*
  ([capture](release-killed-1600x1000.png)).
- *Mark as handled* cleared the notice.

### Scenario 4 (criterion 5): an agent session holds the project

1. Start the release `apiforge mcp` as the client *Claude Code*. Its
   `create_document` takes the project from the clean window.
2. Run the command.
3. Call `finish_editing`.

Observed:
- The command exited 1 after 0.48 s with *Claude Code via apiforge mcp has
  held this project since 22:58; nothing was sent. Run again when it has
  finished editing*.
- The server's stderr stayed empty: no connection reached its endpoint.
- The window observed the agent throughout
  ([capture](release-agent-holds-1600x1000.png)) and was the writer again
  after `finish_editing`.

### Scenario 5 (criterion 9): a simulation with the window open

`apiforge run` without `--live`, with the window holding the project:
- The simulation ran and ended *Failed*, exit 1: the edited `80 * 2` no
  longer matches the sample cart.
- stderr was empty, and the API logged no request.
- Every file in the folder except the held lock had the same SHA-256 before
  and after.

### MCP-10 in the bar

MCP-10 (merged as `153a51e`) shows an agent's run in the bar:
- a status line at the right, *Agent: running promotion.afs (live)…*;
- the holder's words shortened to *<client> · since HH:MM* when the status
  needs the room.

The first release run of the merge showed that line for the command too,
as *Agent: running promotion-and-cart-price.afs (live)…*, which names an
agent that is not there. The merge keeps a command's run out of that
status line. The command's bar is therefore mock-up (a) as approved: its
words in full and no status. The 960 × 540 and 1600 × 1000 captures above
are from this build. The test
`the_bar_of_a_running_command_keeps_its_words_and_no_agent_status` covers
it.

Whether a command's run should have a status line of its own, and in what
words, is a question for the owner. The command still sends its `run`
messages, with MCP-10's step count and first failing step.

### Against the mock-up

| | Mock-up | This branch |
| --- | --- | --- |
| (a) Bar text | *apiforge run --live · since 10:42 · editing is paused here; this window follows every change* | the same, with the real start time; [960](release-bar-code-960x540.png), [1600](release-bar-code-1600x1000.png) |
| (a) Badge, buttons | *Observing*; *Save* and *Save all* disabled; no *Take back* | the same |
| (b) Heading, sentence | *A command asks for this project*; *apiforge run --live wants to send live requests from this project. Unsaved work here:* | the same, wrapped at the same word; [capture](release-prompt-960x540.png) |
| (b) List, buttons | two files, *unsaved code*; *Save all and hand over*, *Not now* | the same |

The release window draws 8 px further left than the headless review
harness, as in earlier reports. The design-review views
`review_mcp15_command_bar` and `review_mcp15_command_prompt` render the
same views in the harness, and they match (a) and (b).

Two things the mock-up does not show, both approved by the owner on
2026-11-01 and recorded in the ticket:
- After *Not now* for a command, the bar does not show MCP-08b's *An agent
  asked to edit this project; save your work first*. No agent asked, and the
  command already has its answer and has ended. An agent's refusal still
  shows the line.
- MCP-09's *A live run is in progress; it will finish. No new one starts.*
  under the agent's toggle is not shown during a command's live run. The
  toggle does not govern the command (decision 3 A).

## Windows

Not run on Windows by hand. The `windows-2022` CI job runs the desktop tests
and the CLI tests of criteria 1, 3–6 and 8–10 over the named pipe.

On `e9407b6`, two of those tests failed there (criteria 5 and 9) only
because the test's folder snapshot read the held lock file (OS error 33).
`6c9eadb` reads it by its length. The run on the final head is in the pull
request's checks. The ticket is not accepted for Windows on this report.

## Not verified

- A **passing** live run in the release binaries. The scratch API answers
  too little for the demo scenario, so the release runs ended *Failed*
  (exit 1). Exit 0 with the window open is covered by CLI criterion 1 and
  by the desktop tests of criteria 1, 2 and 7, which run against the demo
  API.
- A save in the release window after the take-back. The desktop test of
  criterion 1 covers it.
- The waiting line when the window is busy with something other than the
  prompt, such as its own run or a dialog. The line is neutral (*Waiting
  for the API Forge window to answer*, the owner's decision of 2026-11-01),
  so it holds in that case too; it was seen in the release binary with the
  prompt only.
- Ctrl+C at the terminal while the prompt is open, in the release binary.
  The prompt now closes when the command's connection ends. The test
  `a_command_that_stops_waiting_closes_its_prompt` covers the command
  being killed; a terminal's Ctrl+C was not sent.
- A wait beyond 30 s in the release binary. The CLI test of criterion 3
  covers it.
- Criteria 6, 8 and 10 in the release binaries. Their tests drive the real
  `apiforge` binary.
- The `/mnt/<drive>/` sentence on a real WSL mount; it is unit-tested only.
- A window of a build older than MCP-15. Such a window reads the command's
  request as an agent's, so its prompt would say *An agent asks to edit
  this project*. No test runs two builds.
- Keyboard-only operation and a screen reader session.

## Changes after the first review

Fixed:
- A command killed while its prompt is open now closes the prompt (new test
  `a_command_that_stops_waiting_closes_its_prompt`).
- An answer meant for a command that has gone is no longer kept for the
  next agent request. A command is always asked itself, and never answered
  with an agent's earlier choice (`an_answer_for_a_command_that_has_gone_is_not_kept_for_an_agent`).
- The bar's refusal state follows the request that is waiting when *Save
  all* finishes, not the prompt that started it.
- A holder's client name in a refusal is printed on one line, without
  control characters, and cut at 40 characters, as the window's bar does.
  This applies to the server's refusals too. Unit test
  `a_holders_name_cannot_break_the_terminal_line`.
- The backlog's order line, the ticket appendix's pointers to the
  implemented names, and this report's statements about secrets and about
  criterion 3.

Not changed:
- The sprint capacity total. Additions made during the sprint are charged
  to the UX line, as the backlog says.
- The waiting line's wording. It is the ticket's text, and is put to the
  owner.

Nothing drawn changed, and the captures were not retaken.

## Regression smoke list

Linux, 2026-11-01, with the release binaries above:

| # | Result |
| --- | --- |
| 2, 10 | `--screenshot` of the demo shop in Code, Diagram and Split at 960 × 540 and 1600 × 1000: all six captures at the requested size, showing the tree, the editor and the board, with all controls visible (not committed) |
| 6, 7 (partly) | Scenarios 1–2: the run's report was printed; *Save all* wrote the exact edited bytes |
| 1, 3–5, 8, 9 | Not run on the release binary for this ticket; the workspace test suite covers them |

The owner's Windows run is pending, as for the earlier tickets.

## Verification

On `e9407b6`, all exit 0:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1 371 passed, 0 failed,
  40 ignored
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

On `6c9eadb`, the head that carries the Windows and CodeQL fixes, all exit
0:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1 371 passed, 0 failed,
  40 ignored
- the fixture check

On `aca7e14`, after these changes, all exit 0:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1 374 passed, 0 failed,
  40 ignored
- the fixture check

After the merge with MCP-10 (`9549dc1`) and the owner's waiting line, all
exit 0:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1 385 passed, 0 failed,
  42 ignored
- the fixture check
