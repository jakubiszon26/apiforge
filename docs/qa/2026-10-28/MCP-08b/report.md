# MCP-08b: Hand-over on request — QA report

- Branch: `feat/mcp-08b-hand-over`. Captures are from the release binaries
  of `82c5410`.
- Built with: `cargo build --release --locked` (Rust 1.99.0).
- Compared against mock-up: M1 round 4,
  `docs/qa/2026-10-27/MCP-00/m1-observer-*.png` (approved 2026-10-27).
- Verification on `82c5410`, all exit 0:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --all-targets --locked`: 1 258 passed, 0 failed,
    33 ignored
  - `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
  - the UI gate `tools/qa/ui-quality/run.sh` with its own
    `CARGO_TARGET_DIR`: 10 probes passed
- Acceptance tests first: commit `3056d53`. Before the implementation all
  14 failed for the right reason: 13 never saw the window name an endpoint
  in its holder file, and the server-first test never saw *Take back*. Two
  tests changed their procedure afterwards, not their assertions (see the
  implementation commit `47ed5e4`).

## How the release binaries were driven

- **Host.** Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver,
  X11 backend, scratch configuration, data and runtime directories. There
  is no input on this host: the window was never clicked or typed into.
  Screens of a running window were grabbed from the display (1600 × 1000,
  the window's default size); `--screenshot` produced the 960 × 540
  captures.
- **The agent is real.** The release `apiforge mcp` was started beside the
  window and driven over stdio by a scratch script (not committed) as a
  client named *Claude Code*: `initialize`, `read_source`, `write_source`,
  `finish_editing`, end of stdin. The demo shop is the English
  `shop_project` the tests use.
- **Where the harness was used instead.** *Take back* needs a click, and
  the refusal needs work typed in the window. The click, the wait for the
  agent's run, the save in flight and the killed server's recovery are
  covered by the `egui_kittest` acceptance tests in
  `folder/acceptance_tests.rs`, module `mcp08b`, whose server is a real
  process on real stdio. Their captures, named `harness-*.png`, are a
  design review aid, not QA evidence.

## Linux

### Scenario 1 (criteria 1 and 2): a clean window hands the project over

1. Open the shop in the release window; wait until the holder file names
   the window's endpoint.
2. The agent writes the scenario with a comment added.
3. The agent writes again: `80 * 1` becomes `80 * 2`.

Expected: the write succeeds; the window becomes the observer at once,
names *Claude Code*, shows *Take back* at the right end of the bar with its
right edge on *Run*'s, and shows each save within two seconds.

Observed: as expected. The first `write_source` answered `wrote: true` in
0.55 s, the hand-over included; the second in 0.07 s. Before:
[capture](handover-before-1600x1000.png). After the first write, 1.5 s
later: *Observing*, *Save* and *Save all* disabled, the bar *Claude Code via
apiforge mcp · since 08:43 · editing is paused here; this window follows
every change*, *08:43 · promotion-and-cart-price.afs saved*, *Take back*
[capture](handover-observing-1600x1000.png). After the second, 0.8 s later:
line 10 reads `80 * 2` [capture](handover-followed-1600x1000.png). The
comment draws a formatting warning in Problems; that is the language's,
not this ticket's.

Against M1: the badge, the bar, its texts and *Take back* sit where the
mock-up has them; the button is the window's compact action button with the
Undo icon, its right edge at 1584 px like *Run*'s.

**Harness evidence:**
`a_clean_window_hands_over_and_shows_the_save_it_was_told_of` shows the
save within two seconds while the window's own checks are ten minutes
apart, so the `saved` message made it look;
`the_holder_listens_and_a_reader_finds_its_endpoint` connects to both
endpoints found through `ProjectStore::holder()`.

### Scenario 2 (criterion 3): unsaved work refuses

1. Leave unsaved work in the shop's local recovery, as a window closed with
   unsaved work does; open the shop in the release window, which restores
   it.
2. The agent writes the scenario.

Expected: `handover_refused`, nothing written; the window keeps the project
and says why in the bar; the person's text stays.

Observed: as expected. The tool answered in 0.41 s with `code:
handover_refused`, `wrote: false`, `details.reason: "the window has unsaved
work"` and the window as holder. The window stayed the writer with its
unsaved line 2, *Save* enabled, and the bar at the bottom read *An agent
asked to edit this project; save your work first*: before
[capture](unsaved-before-1600x1000.png), after
[capture](refused-1600x1000.png). At 960 × 540 from the harness:
[harness capture](harness-refused-960x540.png).

The refusal bar is the observer's bar, shown while the window is the writer
and only with this text; it goes once the work is saved. M1 does not show
this state (see *Deviations*).

### Scenario 3 (criterion 4): a window opened while the agent holds

1. The agent writes the shop with no window open; it keeps the project.
2. `--screenshot` of the release window in Code, Diagram and Split at
   960 × 540 and 1600 × 1000.

Expected: *Observing*, the bar naming the agent, *Take back*.

Observed: as expected in all six:
[code 960](server-first-code-960x540.png),
[diagram 960](server-first-diagram-960x540.png),
[split 960](server-first-split-960x540.png),
[code 1600](server-first-code-1600x1000.png),
[diagram 1600](server-first-diagram-1600x1000.png),
[split 1600](server-first-split-1600x1000.png). Each window connected to the
server: its stderr has *apiforge mcp: channel: a window disconnected* six
times, once per window closed after its capture.

**Harness evidence:**
`a_window_opened_while_the_agent_holds_connects_and_follows` shows the
agent's later save within two seconds with the window's checks ten minutes
apart.

### Scenario 4 (criteria 5 and 7): the project comes back

1. After scenario 1, the agent calls `finish_editing`.
2. The agent writes again.
3. The agent's stdin ends.

Expected: the window is the writer within two seconds each time; the next
write asks again and a clean window grants it; the server ends after EOF.

Observed: the holder file named the window again 0.05 s after
`finish_editing` answered; the bar and the badge were gone
[capture](taken-back-1600x1000.png). The next write handed over again in
0.54 s [capture](handover-again-1600x1000.png). After EOF the server ended
with exit code 0 and the window held the project 0.05 s later
[capture](taken-back-eof-1600x1000.png).

**Harness evidence:** *Take back* itself, clicked:
`take_back_while_the_agent_is_idle_and_the_next_write_asks_again`
(writable within two seconds, the next write granted again);
`finishing_or_disconnecting_gives_the_project_back_within_two_seconds`.

### Scenario 5 (criterion 6): *Take back* during an operation

Harness only (it needs a click):
`take_back_waits_for_the_agents_run_and_its_cleanup` starts a slow
simulation and presses *Take back*: the bar reads *Claude Code via apiforge
mcp · since 08:44 · waiting for the agent's run to finish*, *Take back* is
greyed, the window stays the observer until the run's answer arrives with
its cleanup steps, then is writable within two seconds
[harness 960](harness-waiting-960x540.png),
[harness 1600](harness-waiting-1600x1000.png).
`take_back_during_a_write_lets_it_commit_or_fail_whole` presses it while a
write is in flight: the disk holds the old text or the new one whole,
matching `wrote`.

### Scenario 6 (criterion 8): the server dies

1. The window hands the shop over; the agent's process is killed.

Expected: the window takes the project back.

Observed: the holder file named the window 0.05 s after the kill
[capture](killed-taken-back-1600x1000.png).

**Harness evidence:**
`a_killed_server_leaves_its_transaction_to_the_recovery_view` leaves an
activated transaction before the kill: the window opens the recovery view
and finishes it.

### Scenario 7 (criteria 9 to 12)

Harness and stdio only:
- `a_second_agent_is_refused_with_the_first_ones_description`:
  `held_by_other` naming *Claude Code via apiforge mcp*; the window still
  observes the first.
- `closing_the_observing_window_leaves_the_agent_writing`: the next write
  succeeds; *apiforge mcp: channel: a window disconnected* is on stderr,
  nothing but protocol on stdout.
- `a_long_pause_between_writes_changes_nothing`: fifteen check intervals
  without a write; still the observer; the next write succeeds.
- `the_channel_is_private_and_ignores_what_it_cannot_read`: the endpoint is
  `apiforge-` and 32 hex digits; the holder file has no path; the socket is
  mode 0600 in a 0700 directory; non-JSON, another version, an unknown type
  and a truncated message are ignored, logged by kind, and both ends keep
  working.

The QA run itself found one defect, fixed in `82c5410`: a runtime
directory deep enough to put the socket path over the system's limit left
both ends without an endpoint. Over 100 bytes the temporary directory is
used, by the same rule on both sides.

## Review round 1

`f804722` answers the automated review of #114 (cubic) and one Windows CI
failure; the workspace suite passed again on Linux (1 258 passed, 0 failed,
33 ignored). The release-binary captures were not retaken: nothing drawn
changed.

- **Windows CI, run 40660700539.** `a_clean_window_hands_over_and_shows_the_save_it_was_told_of`
  got `recovery_required`: right after the grant the window's check read
  the files the agent's first commit was replacing, and Windows refused the
  replace after the commit's activation. The window now grants only with no
  check in flight and postpones its next check; the agent's `saved` makes
  it look. The same test had passed on Windows in the pull request's own
  run. A check that meets a later commit is the observer's existing risk on
  Windows (MCP-08a) and is not changed here.
- A request retried after a stale release is answered only for the same
  project, never after a newer request or for an agent that stopped
  waiting; opening another project refuses a waiting request.
- The server tells `released` before a waiting write can start a new
  session, and only to the windows it lets go.
- The socket base falls back to `/tmp` when the temporary directory is too
  long as well.
- Not changed: the Windows pipe's default access (restricting it needs
  `unsafe`), and the window's channel thread sending one message at a time
  (each bounded to two seconds, same-user peers).

## After acceptance: a race in the take-back during a write

Windows push run 40669262333 on `6b096d3` failed
`take_back_during_a_write_lets_it_commit_or_fail_whole` ("never showed a
settled holder"); the pull request's run of the same commit passed. A
product race, fixed in `ea67b0a`: when the server handled *Take back*
before the write that was in flight, the session ended and that write found
the lock free and took the project again without asking, before the window
could take it back. The window, told `released`, reconnected to the agent
but kept its pending take-back, so *Take back* stayed greyed with no way to
ask again.

- The server: a session ended by *Take back* lets the window take the free
  lock first; the next write, also one already waiting, waits up to 3 s for
  the window to hold the project, then asks it like any other write
  (criterion 5).
- The window: `released` or a lost connection answers a pending take-back,
  so *Take back* is offered again should the agent hold the project again.

`origin/main` with MCP-06 was merged after the fix (`21872fc`). On the
merged head the MCP-08a and MCP-08b tests passed 20 times in a row (22
tests each run), and the workspace suite passed: 1 286 passed, 0 failed,
33 ignored, MCP-06's tests among them. The merged run also showed a race in
an MCP-08a test, `a_document_the_holder_registers_is_listed_without_a_notice`:
its wait used `query_by_label_contains`, which panics when more than one
node matches, and once the commit is followed the bar's last change
contains the file's name as well as the explorer's row. The wait now finds
the row by its exact name (`query_by_label`), which the bar's longer text
cannot match; its assertions are unchanged.

The pre-merge review then found that the hand-over's first fix for Windows
(`f804722`: grant only with no check in flight, then postpone the next
check) left the race open. A new test,
`a_granting_window_reads_nothing_until_the_agent_has_saved`, with checks due
every 20 ms, failed three runs of three on that code: checks started
between the window's decision and the agent's `saved`, the first in the
same frame as the decision, and once a check was in flight at the grant.
Since the following commit the window holds every check from its decision
to hand over until the agent's first `saved` (or `released`, the lost
connection, or ten seconds, the bound the window already gives a grant):
nothing can read the files the agent's first commit replaces.

## Windows

From CI run 40656947002 of this branch at `47ed5e4` (windows-2022), the
implementation before the socket-path fix, which changes Unix only:

- `cargo test --workspace --all-targets --locked`: 1 247 passed, 0 failed,
  33 ignored (the difference to Linux is other tickets' Unix-only tests).
- **The named pipe works on Windows.** All 14 MCP-08b acceptance tests
  passed there, with the window listening on `\\.\pipe\apiforge-<random>`
  and the server process spawned over real stdio: the hand-over, the
  refusal, the server-first connection, *Take back* idle and during a run
  and a write, `finish_editing` and EOF, the killed server's recovery, the
  second agent, the closed window, the pause, and criterion 12's connection
  to both pipes (opened as a file) with lines that are not messages. The
  channel crate's listener and client tests passed too.
- The pipe's access on Windows is the system default (see *Not verified*).
- No release-binary run by hand on Windows; the owner's Windows run is
  pending. The folder-project captures of that CI run show the window
  starting as before.

## Deviations from the ticket and the mock-up

- **The refusal in the bar.** Criterion 3 says the window shows the refusal
  "in the bar"; M1's bar exists only while observing. The window shows the
  same bar, without *Take back* or a last change, while it is the writer and
  has refused for unsaved work, until the work is saved. MCP-08c replaces
  this with the prompt of M2.
- **Where the wait is said.** *waiting for the agent's run to finish*
  replaces *editing is paused here; …* at the end of the holder's text; M1
  has no such state.
- **A request while the window is busy** (storage work, its own run, any
  dialog, a project switch) waits until the window is idle; after 30 s the
  agent gets `handover_pending` and its next request is answered at once.
- **The stdio helper of MCP-01's tests** is shared as
  `apiforge_mcp::test_support` (feature `test-support`) and used by the
  desktop tests; the CLI's own tests keep their copy, to stay clear of
  MCP-06's work on the same file.
- **The server in the desktop tests** is the desktop test executable
  serving `apiforge mcp` from an ignored helper test (the spawned-helper
  exception), because a test of one package cannot name another package's
  binary.
- **A new crate**, `apiforge-channel`, holds the protocol for both ends.

## Not verified

- **Windows named pipes by hand**, and the release binary on Windows; see
  *Windows*.
- **Claude Code itself**: the agent here is a script speaking the same
  protocol revision. With the release binary registered as the guide says,
  a write should hand a clean window over and show *Take back*.
- **On Windows the pipe's access is the system default**: the creating
  user, administrators and LocalSystem have full control, and everyone else
  may open it for reading. Restricting it needs a security descriptor,
  which needs `unsafe` code this workspace forbids. Remote clients are
  refused.
- **A killed process leaves its socket directory** under the runtime
  directory (`apiforge-<random>`); a closed one removes it. The smoke run's
  SIGTERM left one.
- **The 960 × 540 hand-over in a running window**: the release window opens
  at 1600 × 1000 and this host cannot resize it; 960 × 540 is covered by the
  `--screenshot` captures of scenario 3 and the harness.

## Regression smoke list

Linux, 2026-10-28, release binaries of `82c5410`, on Xvfb (X11, Lavapipe),
scratch data and configuration directories.

| # | Result |
| --- | --- |
| 1 | Pass: no arguments created and opened the managed demo shop; nothing on stderr in 15 s. |
| 2 | Pass: `--folder` showed the explorer and the scenario in Code (scenarios 1 to 3). |
| 7 | Pass for the agent's saves: each `write_source` left the bytes it was given and the window reloaded them. Ctrl+S: not run, no input on this host; the harness tests save after a take-back. |
| 10 | Pass: the 960 × 540 captures of scenario 3 show every control in Code, Diagram and Split, with the bar and *Take back*. |
| 3–6, 8, 9 | Not run by hand: no input on this host. The workspace suite that covers them passes. |
