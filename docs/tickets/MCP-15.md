# MCP-15: A live run from the command line asks the open window for the project

- Size: M · Owner review: 30 min · Risk: `default`, `visible UI`
- State: Implemented, awaiting acceptance (issue #182) · owner decision of 2026-11-01: the
  command line asks the window over the control channel, as the server does,
  instead of refusing · decisions 1–6 taken 2026-11-01 on #182, all A ·
  mock-up (a) and (b) approved as a whole on 2026-11-01 on #182, captures in
  [`docs/qa/2026-11-01/MCP-15/`](../qa/2026-11-01/MCP-15/) · after MCP-12
  (#123, merged in #178: endpoint discovery, reused here)
- Analysis model: Fable · Implementation model: Claude Opus 5.5
- Analysis: this file; the mechanism is [MCP-00](MCP-00.md#hand-over-between-the-window-and-the-agent)
  and [ADR 0014](../decisions/0014-agent-sessions.md) decision 4

## Goal

With the project open in the window, `apiforge run --folder <p> --scenario
<s> --live` runs instead of answering *The folder project is open in another
writer session*: the command asks the window for the project the way the MCP
server does, the window hands it over and observes, the run sends and
journals its requests as today, and when the command ends the window is the
writer again — nothing closed, nothing reopened.

## What the owner will see

- New, in the terminal: with a clean window open, the live run proceeds; one
  line on stderr says *The open API Forge window handed the project over for
  this run*. With unsaved work in the window the command waits and says
  *The API Forge window has unsaved work and asks you what to do; answer
  there* (decision 2 A). A refusal names the holder in the server's words:
  *Claude Code via apiforge mcp has held this project since 10:42; nothing was
  sent. Run again when it has finished editing* (an agent, decision 5 A);
  *apiforge apply has held this project since 10:41; nothing was sent. Run
  again when it has finished* (another command); *The API Forge window has
  held this project since 10:40 and could not be asked; nothing was sent.
  Close the project there or update API Forge, then run again* (a window
  with no channel: an older build, or an endpoint that cannot be reached,
  with MCP-12's Windows/WSL sentence when the project is under
  `/mnt/<drive>/`). Exit code 1 for every refusal, as today.
- New, in the window: while the command holds the project the bottom bar
  reads *apiforge run --live · since 10:42 · editing is paused here; this
  window follows every change* — M1's wording for a command, with the
  subcommand as typed — and the *Observing* badge shows; *Take back* is not
  offered (decision 1 A). With unsaved work, the MCP-08c prompt opens with
  the heading *A command asks for this project* and the sentence *apiforge
  run --live wants to send live requests from this project. Unsaved work
  here:* above the same list and the same two buttons (decision 2 A).
- Changed: the refusal texts above replace *The folder project is open in
  another writer session* for a live folder run. The window's `granted` path,
  bar and prompt accept a command as the asker, not only an agent.
- Removed, hidden, migrated: nothing. Simulations (`run` without `--live`),
  `check`, `print` and the `--workspace` mode are untouched; `apply` keeps
  today's refusal (decision 6 A).
- Default behaviour that changes: a live folder run with a clean window open
  **runs** instead of refusing, and the window becomes an observer for the
  run's duration without a prompt (MCP-00 decision 11 applied to the person's
  own command). The person's `--live` is the live consent; the window's
  per-project toggle (MCP-09) does not govern it (decision 3 A). The window
  is the writer again at the command's end (decision 4 A).

## Mock-up

**Approved on 2026-11-01** as a whole (#182): (a)
[`a-command-bar-code-960x540.png`](../qa/2026-11-01/MCP-15/a-command-bar-code-960x540.png),
[`a-command-bar-code-1600x1000.png`](../qa/2026-11-01/MCP-15/a-command-bar-code-1600x1000.png)
and (b) [`b-command-prompt-960x540.png`](../qa/2026-11-01/MCP-15/b-command-prompt-960x540.png),
rendered from `mock/mcp-15-cli-handover` at `db71eec`. The request as written
before approval:

Required before approval, small: both views exist and are approved
(M1, M2); only their words change for a command. Form 1 preferred — a
`review_*` capture from a throwaway branch of (a) the bottom bar with
`Holder::command("run --live")`, the *Observing* badge and no *Take back*,
in Code at 960 × 540 and 1600 × 1000; (b) the hand-over prompt with the
command's heading and sentence and two unsaved items at 960 × 540. Form 2
(the `m1-*`/`m2-*` captures annotated with the new strings) is acceptable.
Posted to #182; the approved captures go under `docs/qa/<date>/MCP-15/`.
Both (a) and (b) are needed: decision 2 = A keeps the prompt.

## Decisions

Taken by the owner on 2026-11-01 on #182: **1–6 = A**, the recommended
option in each. The rules the ticket now follows:

1. While a command holds the project the window is an observer as for every
   holder: the bar reads *apiforge run --live · since HH:MM · editing is
   paused here; this window follows every change*; **no *Take back*** for a
   command holder.
2. With unsaved work the window shows the MCP-08c prompt naming the command;
   *Save all and hand over* saves and the run proceeds; *Not now* or Escape
   keeps the person editing and the command exits 1 with *The window kept
   the project (you chose to keep editing); nothing was sent*. The command
   waits as long as the prompt is open; Ctrl+C gives up with nothing sent.
3. The person's own `--live` is the live consent. The MCP-09 toggle governs
   the agent only; a `permission` message from the window is ignored by the
   command.
4. The window takes the project back at the command's end, every end: the
   command releases the lock and sends `released`; a killed command is taken
   back by the window's probe, with the interrupted-run notice as at start.
5. An agent session holding the project is refused by name, exit 1, nothing
   sent, no request to the agent's server.
6. Scope is `run --live` only; `apply` keeps today's refusal and gets its own
   S ticket if the owner wants it.

The options as they were put, for the record (A recommended in each):

1. **What the window shows while a command holds the project.** **A:** the
   observer state as for every holder (MCP-08a): the bar with M1's wording for
   a command, *apiforge run --live · since 10:42 · editing is paused here; this
   window follows every change*, and **no *Take back***: a live run is never
   cut mid-way (decision 16, ADR 0009), so the button could only wait for the
   end that comes anyway. **B:** the same bar with *Take back*, which waits
   for the run to finish, cleanup included, saying *waiting for the run to
   finish* as for an agent. Consequence of B: a button whose only effect is
   what happens without it.
2. **Unsaved work when the command asks.** **A:** the MCP-08c prompt, with the
   command named instead of an agent: *Save all and hand over* saves and the
   run proceeds; *Not now* or Escape keeps the person editing and the command
   exits 1 with *The window kept the project (you chose to keep editing);
   nothing was sent*. The command waits for the answer as long as the prompt
   is open (Ctrl+C gives up; nothing was sent). **B:** refuse as MCP-08b did:
   *The API Forge window has unsaved work; save it there, then run again*,
   no prompt. Consequence of B: the person saves by hand and types the
   command again; the window shows nothing.
3. **Live consent for the command's run.** **A:** the person's own `--live` is
   the consent, as it is today and as the window's own *Run live requests* is;
   the MCP-09 toggle (*Allow the MCP agent to send live requests*) governs the
   agent only, and a `permission` message from the window is ignored by the
   command. **B:** while a window is connected the command obeys the toggle
   too and is refused with *live requests are off in this project's settings*
   when it is off (default off). Consequence of B: a command the person typed
   is refused by a setting written for the agent, and until MCP-09 ships the
   command could never run with a window open.
4. **When the window takes the project back.** **A:** at the command's end,
   whatever the end: the command releases the lock and says `released` over
   the channel, so the window is the writer within a second; passed, failed
   and a `Not now` alike; a killed command (Ctrl+C) leaves the lock to the OS
   and the window takes it back by its probe and reports the interrupted run,
   as for a dead server (MCP-07 criterion 5). **B:** no `released`: the
   window relies on its once-a-second probe alone. Consequence of B: up to
   two seconds later, no other difference.
5. **An agent session holds the project.** **A:** refuse, naming the agent as
   the server names a second agent — *Claude Code via apiforge mcp has held
   this project since 10:42; nothing was sent. Run again when it has finished
   editing* — exit 1, nothing sent, no request to the agent's server (MCP-00:
   no hand-over between agents; ADR 0014 decision 1: a session ends only by
   `finish_editing`, EOF or death). **B:** ask the agent's server over its
   endpoint; it would have to end the model's session for a command.
   Consequence of B: the model loses the project mid-task, which revision 2
   of MCP-00 was written to prevent.
6. **Scope: `apply` too?** **A:** `run --live` only, the owner's finding;
   `apply` keeps *The folder project is open in another writer session* and
   gets its own S ticket if wanted, on the same client. **B:** `apply` asks
   the window the same way in this ticket (bar: *apiforge apply · since …*,
   the window follows the saved file by `saved`). Consequence of B: one more
   path and test here, about 5 more review minutes, and a save from a script
   can take the project from a clean window without a prompt.

## Acceptance criteria

1. **Clean window first.** The window is open and clean on a project whose
   base URL is the local demo API; `apiforge run --folder … --scenario …
   --live` hands over (the bar shows the command's wording of decision 1 A
   and no *Take back* at any point of the run, the badge reads *Observing*),
   the requests are sent and journalled under `.apiforge/local/`, the report
   is printed, exit 0; within two seconds of the command's exit the window is
   the writer again and a save there succeeds without a conflict. Desktop interaction test in
   `folder/acceptance_tests.rs` (the command run through the spawned-helper
   path of MCP-08b); CLI test in `crates/apiforge-cli/tests/folder.rs`
   against a scripted window-role listener.
2. **Unsaved work** (decision 2 A): with unsaved text in two documents the
   prompt opens with the command's heading and lists both; *Save all and hand
   over* saves and the run proceeds; *Not now* exits the command with code 1
   and the message, sends nothing, the window stays the writer and the text is
   untouched. Desktop test.
3. **The command waits for the window's answer**: a listener that answers
   after a delay longer than the server's 30 s limit is still waited for and
   granted (CLI test with the delay as a test seam on the listener side);
   while waiting more than two seconds the command says so once on stderr.
4. **Live consent** (decision 3 A): a `permission { live: false }` sent by the
   listener after `granted` changes nothing; the run sends. CLI test.
5. **An agent holds** (decision 5 A): with the lock held by
   `Holder::new(Agent, Some("Claude Code"))` that names an endpoint, the
   command exits 1 with the agent's refusal text, sends nothing and opens no
   connection to that endpoint (the listener sees none). CLI test.
6. **Other holders are named**: `Holder::command("apply")`, a window with no
   endpoint, an unreadable description (unknown holder): each refusal text of
   *What the owner will see*; the `/mnt/<drive>/` sentence appears only for an
   unreachable window on such a path. CLI tests; the existing
   `a_live_folder_run_requires_its_project_lock_and_a_writable_journal` keeps
   passing with its wording assertion updated.
7. **Every end releases** (decision 4 A): passed and failed exit 0/1 and the
   window is the writer within two seconds; the command killed mid-run (a
   child in the test, the demo API answering slowly) leaves its journal entry
   and the window takes the project back and shows the interrupted-run notice
   as at start. Desktop tests.
8. **The window closes while the command runs**: the command finishes and
   exits 0; the lost channel is one stderr line. CLI test (the listener drops
   the connection after `granted`).
9. **A simulation never asks**: `run` without `--live` with the window open
   opens no connection, takes no lock, and the project folder is
   byte-identical afterwards. CLI test with a listener that fails the test on
   any connection.
10. **The messages**: the listener receives, in order, `request { client:
    "run --live", pid, role: "command" }`, then after `granted` `run {
    mode: live, state: running }`, `run { state: passed | failed }`,
    `released`, then EOF. CLI test. (MCP-10 shows these in the bar once it
    lands; nothing of it is in this ticket.)
11. **Windows**: the CLI tests of criteria 1, 5, 8 and 10 run on the
    windows-2022 CI job over the named pipe; the QA report records the run or
    states the absence of evidence item by item.
12. The development guide (*Folder commands*, *The window and the agent*)
    says what a live folder run does with a window open; the QA report
    compares the bar and the prompt against the approved captures; the
    verification commands of the development guide pass.

## Implementation (2026-11-01)

Claude Opus 5.5, on this branch after the acceptance commit. The criteria and
their tests:

| # | Test |
| --- | --- |
| 1 | CLI `mcp15_criterion_1_a_clean_window_hands_over_and_the_run_sends`; desktop `mcp15_criterion_1_a_clean_window_hands_over_and_takes_the_project_back` |
| 2 | desktop `mcp15_criterion_2_save_all_saves_and_the_run_proceeds`, `mcp15_criterion_2_not_now_keeps_editing_and_sends_nothing`; CLI `mcp15_a_window_that_keeps_the_project_sends_nothing` |
| 3 | CLI `mcp15_criterion_3_the_command_waits_for_the_windows_answer` (the scripted window answers after 32 s) |
| 4 | CLI `mcp15_criterion_4_the_windows_live_toggle_does_not_govern_the_command` |
| 5 | CLI `mcp15_criterion_5_an_agent_session_is_refused_by_name_and_not_asked` |
| 6 | CLI `mcp15_criterion_6_other_holders_are_named`, the updated `a_live_folder_run_requires_its_project_lock_and_a_writable_journal`; unit tests of `apiforge_mcp::command` for the wording and the `/mnt/<drive>/` sentence |
| 7 | desktop `mcp15_criterion_7_a_passed_and_a_failed_run_give_the_project_back`, `mcp15_criterion_7_a_killed_command_is_taken_back_with_its_interrupted_run` |
| 8 | CLI `mcp15_criterion_8_the_window_closing_mid_run_changes_nothing` |
| 9 | CLI `mcp15_criterion_9_a_simulation_never_asks` |
| 10 | CLI `mcp15_criterion_10_the_messages_in_order` (passed and failed) |
| 11 | the CLI tests above run on the `windows-2022` job; the QA report records the run |
| 12 | *Folder commands* and *The window and the agent* in the development guide; ADR 0014's MCP-15 amendment; the QA report |

As the appendix proposed: `apiforge_mcp::command::run_live` holds the live
folder run, called by the CLI and by the desktop's spawned helper; the
asking is `handover::request_project`, shared with the server; the refusal
wording sits beside the server's, sharing the agent's and the command's
names. The CLI reads the secrets file first and `run_live` reads the project
before anything else, so neither a secrets file that cannot be read nor a
scenario that is not registered asks a window. `run_live` takes the secret
values and a callback for the journal's interrupted runs, and returns the
`RunOutcome` the CLI prints. A command that stops waiting (Ctrl+C) while the
prompt is open closes the prompt; an answer meant for a command that has
gone is not kept for a later agent (review of #184).

Two consequences of decisions 1–3 that the analysis did not spell out,
reported here for the owner:

- After *Not now* for a command the bottom bar does not show MCP-08b's *An
  agent asked to edit this project; save your work first*: no agent asked,
  and the command has its answer and has ended. An agent's refusal shows it
  as before. Test `not_now_for_a_command_leaves_no_agent_wording_in_the_bar`.
- MCP-09's *A live run is in progress; it will finish. No new one starts.*
  under the agent's toggle is not shown for a command's live run, which the
  toggle does not govern (decision 3 A).

## Not in scope

`apply` asking the window (decision 6 A: a later S ticket, on the same
client); the `--workspace`
JSON mode; a SIGINT handler that cancels the run and runs its cleanup (today
Ctrl+C kills the process and the window reports the interrupted run); the
run status line in the bar (MCP-10; the messages are sent here); the
command's report in *Run results*; a command that listens (a window opened
during the run observes by the holder file and the probe, as for `apply`);
a hand-over between agents; MCP-09's toggle governing the command
(decision 3 A); the WIP and Windows evidence of MCP-12 itself.

## Technical appendix

### Reuse: the server's asking, MCP-12's discovery

- `agent.rs::take_lock` / `ask` and `handover.rs::Hub::ask` already do the
  whole dance: probe → `holder()` → a window with an endpoint is asked with
  `Message::Request`, `granted` is followed by `open_exclusive_as` retried
  for `GRANTED_LOCK`, `refused`/`pending`/unreachable become the result, a
  third process that took the lock meanwhile is `held_by_other` and the
  window is told `released` (`Hub::decline`). The refusal wording lives in
  `agent.rs::held`, `refused`, `pending`. *(Implemented as
  `handover::request_project`, the request-and-answer loop without a
  deadline of its own, the server bounding it to thirty seconds; see
  *Implementation*.)* **Extract** the asking into a public
  function of `apiforge_mcp::handover` — `ask_window(endpoint, owner_uid,
  Request, deadline: Option<Duration>) -> Answer` on a given `Handle` — and
  the wording into `pub fn` texts that both the tool errors and the CLI's
  stderr use, so the server and the command say the same thing. The server
  passes `Some(ANSWER)`; the command passes `None` (criterion 3): a person at
  a terminal waits for the person at the window, the model does not.
- MCP-12 (#178) changes `apiforge_channel::connect(endpoint, owner_uid)` and
  adds `Holder::owner_uid()` plus the `/mnt/<drive>/` hint in `agent.rs`;
  the command must use that `connect` or a window listening under
  `/run/user/<uid>` is unreachable from a terminal without `XDG_RUNTIME_DIR`.
  Hence *after MCP-12*: the branch starts from `main` once #178 is merged.
- `ProjectSession::from_store_saved(store)` (used by `server.rs::run_live`)
  builds the session from the store the hand-over returned; the rest of
  `folder.rs::execute_run` (snapshot, secrets, `JournalStore::open_project`,
  `executor::execute`) is unchanged.

### Where the command's path lives

`folder.rs::execute_run` is a function of a binary crate; the desktop's
acceptance tests cannot call it, and the MCP-08b tests run the server from an
ignored helper test for the same reason. Move the live folder run's body
into a library function — `apiforge_mcp::command::run_live(folder, scenario,
secrets, json_sink)` returning the report *(implemented as `run_live(folder,
scenario, secret values, journal callback) -> Result<RunOutcome, String>`; see
*Implementation*)* — called by the CLI (which prints)
and by a spawned helper test in `apiforge-desktop` (criteria 1, 2, 7). The
CLI's own tests drive the real binary (`CARGO_BIN_EXE_apiforge`) against a
scripted listener: `ProjectStore::open_exclusive_as(.., Holder::window()
.with_endpoint(..))` held by the test, `apiforge_channel::Listener`, and on
`request` the test drops the store and answers `granted` as the window does
(`folder/handover.rs::release`). The test's answer delay (criterion 3) is a
parameter of the listener, not an environment seam in the product.

### The channel: one additive field

`Message::Request { client, pid }` gains `role: Option<HolderRole>` (serde
default `None` = agent, so an older peer reads the message unchanged: the
decoder ignores unknown fields and the test
`what_is_not_a_message_of_this_version_is_ignored_with_a_reason` proves
it). The command sends `client: Some("run --live")`, `role:
Some(Command)`, and writes `Holder::command("run --live")` into the holder
file (today `"run"`), so the bar and the refusal of a third process read
the same name. No new message kind; `version` stays 1.

### The window's side (desktop)

- `handover.rs::read` → `Event::Request { link, client, role }`;
  `handed_over` builds `Holder::new(role, client)` instead of hard-coding
  `HolderRole::Agent`; `holder_seen` resets `granted_at` for a `Command`
  holder too (today only an agent ends the grant window `GRANT`, so a
  command that ended early would be taken back only after `GRANT`); the bar
  offers *Take back* only when the linked holder is an agent (decision 1 A:
  `is_linked() && role == Agent`); `connect_to_holder` stays agent-only (a
  command has no endpoint).
- `handover_prompt.rs`: `HEADING` and the sentence come from the asker's
  role: `holder::command_name(client)` beside `agent_name`; `Prompt::Handover
  { client }` carries the role. Everything else — the list, `save_all_then`,
  `answer_waiting`, `KEEP_EDITING` — is reused as is.
- `holder.rs::holder_text` already renders `(Command, Some(name))` as
  *apiforge {name}*; no change beyond the test strings.
- `Message::Run` / `Released` on the link are handled today (`agent_running`,
  `let_go`); `Permission` on the command's connection is sent by MCP-09's
  window and ignored by the command (decision 3 A).

### The command's lifecycle

1. `--live`: `open_saved_exclusive_as(folder, Holder::command("run --live"))`.
   `Locked` → probe `holder()`: window with endpoint → ask (a current-thread
   Tokio runtime for the duration of the command); window without endpoint,
   command, agent, unknown → the shared refusal text, exit 1, nothing sent.
2. Granted → `open_exclusive_as` retried within `GRANTED_LOCK` (the window
   holds `granted_at` for `GRANT` so its probe does not take the free lock
   back meanwhile); stderr: the hand-over line. Locked past the deadline →
   `released` to the window, refusal naming the third process.
3. Keep the connection; a reader task logs `take_back` (ignored under
   decision 1 A) and `permission`; send `run { running }` before the first
   request and `run { passed | failed | cancelled }` after the report.
4. End: drop journal and session (the lock), send `released`, drop the
   connection, print, exit. A connection lost at any point changes nothing
   (criterion 8), as for the server (MCP-08b criterion 10).
5. No listener in the command: `Holder::command` names no endpoint; a window
   opened during the run reads the holder file (*apiforge run --live · since
   …*), observes, and takes the project back by its probe (MCP-07).

### Why M

Complexity: the change crosses `apiforge-cli`, `apiforge-mcp`,
`apiforge-channel` and the desktop and touches the hand-over protocol —
bounded, because every piece exists and only the asker's identity is new.
Regression potential: the window's hand-over state machine (MCP-08b/c) gains a
second asker; covered by the existing fourteen MCP-08b tests plus criteria
1, 2 and 7. Verification cost: a spawned command in desktop tests, a scripted
window in CLI tests, Windows over CI. Expected 600–900 changed lines, most of
them tests. Review: the owner tries one command against the open window and
reads two captures — 30 minutes.

### Work in progress at analysis

Open pull requests on 2026-11-01: #178 (MCP-12, 40 min) and #179 (REL-03,
10 min), two awaiting acceptance within 50 minutes; this draft is an analysis
and starts no implementation until #178 is merged and the owner approves the
analysis with its mock-up. The six decisions were taken the same day; the
mock-up is what approval still waits for.
