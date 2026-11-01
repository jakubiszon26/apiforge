# MCP-10: The agent's run status in the bottom bar — QA report

- Branch: `feat/mcp-10-run-status`. Captures are from the release binaries
  of `c2d356c` (the implementation; later commits on the branch change
  only this report and its captures).
- Built with: `cargo build --release --locked` (Rust 1.99.0).
- Compared against mock-up: M4 round 4,
  `docs/qa/2026-10-27/MCP-00/m4-run-running-960x540.png` and
  `m4-run-passed-960x540.png` (approved 2026-10-27).
- Analysis model: Fable (MCP-00 revision 2). Implementation model: Claude
  Opus 5.5, taken under [autonomy](../../../process/autonomy.md)
  (`ready:sol`).
- Acceptance tests first: commit `14416eb`. Before the implementation all
  five `mcp10_criterion_*` tests failed for the right reason: the channel
  test *the window was told no run message*, the window tests *the window
  never showed the running line* (or *the passed line*). The
  implementation commit changed one tolerance: a whole label may measure
  up to 1 px narrower than its unplaced layout (glyphs snap to pixels; it
  measured 319.2 of 320.1 px), so `+ 0.5` became `+ 1.0`; a cut label
  loses at least a character (about 5 px). No other assertion changed.

## Criteria and their evidence

| # | Criterion | Test (`folder/acceptance_tests.rs`, `mod mcp10`) | Release binary |
| --- | --- | --- | --- |
| 1 | `run { path, mode, state }` at the start and at the end (`passed`, `failed { step, label }`, `cancelled`); the window shows the line within a second of each | `mcp10_criterion_1_the_server_tells_a_window_each_runs_start_and_end`, `mcp10_criterion_1_the_window_shows_the_line_within_a_second_of_each_message`; also `a_window_that_connects_during_a_run_hears_that_it_runs` | Scenarios 1–3 |
| 2 | The line shows the mode | `mcp10_criterion_2_a_live_run_says_live` (and *(simulation)* in the tests of criterion 1) | Scenario 3 |
| 3 | Never cut at 960 × 540 in Code, Diagram, Split; the holder text gives way; nothing above the bar moves | `mcp10_criterion_3_the_status_is_whole_at_960_by_540_and_the_holder_gives_way` (running, passed and failed, each in all three modes) | Scenario 4 (Code only) |
| 4 | Gone when the channel drops or the agent finishes | `mcp10_criterion_4_the_status_goes_with_the_agent_and_with_the_channel` (`finish_editing` after a run; the server killed during a run) | Scenario 5 (`finish_editing`) |
| 5 | QA against M4; verification passes | this report | — |

The server in these tests is a real process on real stdio, spawned as in
`mod mcp08b`. Criterion 1's channel test connects to the server's endpoint
as a window does and reads the raw JSON lines, so the wire format itself
is asserted:

```json
{"version":1,"type":"run","path":"scenarios/promotion-and-cart-price.afs","mode":"simulation","state":"running"}
{"version":1,"type":"run","path":"scenarios/promotion-and-cart-price.afs","mode":"simulation","state":"passed","step_count":18}
{"version":1,"type":"run","path":"…","mode":"simulation","state":"failed","step":15,"label":"Promotional price","step_count":18}
```

The state that starts a run is `running`, the value MCP-08b's channel
already sends; the ticket's `started` names the same moment (see the pull
request's questions for the owner).

## How the release binaries were driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
scratch configuration, data and runtime directories, no window manager and
no input device: the window was never clicked or typed into. The release
`apiforge-desktop --folder <demo shop>` opened the project as the writer;
the release `apiforge mcp --folder <demo shop> --allow-live` was driven
over stdio by a scratch script (not committed) as the client *Claude
Code*: `initialize`, `read_source`, `write_source`, `run`,
`finish_editing`. The server ran with the test seam
`APIFORGE_MCP_TEST_STEP_DELAY_MS=400` so that a run lasts long enough
(about 4.5 s) to grab while it runs. The release `apiforge-demo-api`
served the live runs on its default port. The screen was grabbed 2.5 s
after each `run` started and 1.5 s after it was answered; the 960 × 540
window is the same process resized through Xlib. The demo shop is the one
`prepare_project` writes, with no local state.

## Linux

### Scenario 1 (criterion 1): a simulation, observed at 1600 × 1000

1. The agent writes the scenario with a comment added; the clean window
   hands the project over and observes.
2. The agent runs the scenario (simulation).

Expected: while it runs, *Agent: running promotion-and-cart-price.afs
(simulation)…* at the right of the bar, left of *Take back*, in place of
the last change; then *Agent: promotion-and-cart-price.afs passed · 18
steps · HH:MM*.

Observed: as expected. Before the run the bar shows the last change
([capture](observing-1600x1000.png)); running
([capture](simulation-running-1600x1000.png)); passed, *18 steps · 14:52*,
the run's `step_count` being 18 ([capture](simulation-ended-1600x1000.png)).
At 1600 px the holder keeps its full text.

### Scenario 2 (criterion 1): a later save, then a failing run

1. The agent writes `expected_price = 81 * 1`.
2. It runs the scenario.

Expected: the save brings the last-change line back in place of the ended
run; the run ends *failed at step N · Promotional price*.

Observed: *14:51 · promotion-and-cart-price.afs saved* after the save
([capture](saved-after-run-1600x1000.png)); then *Agent:
promotion-and-cart-price.afs failed at step 15 · Promotional price · 14:51*
([capture](failed-ended-1600x1000.png)). Step 15 counts every step, a
component's own steps included, as the 18 of `step_count` do.

### Scenario 3 (criterion 2): live runs

1. The agent restores the price and runs with `live: true` (the server
   started with `--allow-live`, the demo API listening).
2. At 960 × 540 it runs live again with the price at 81 (after
   Scenario 4).

Expected: *Agent: running promotion-and-cart-price.afs (live)…*, then the
end with *(live)* after the file's name.

Observed: running live ([capture](live-running-1600x1000.png)); *Agent:
promotion-and-cart-price.afs (live) passed · 18 steps · 14:52*
([capture](live-ended-1600x1000.png)); at 960 × 540 *Agent:
promotion-and-cart-price.afs (live) failed at step 15 · Promotional price ·
14:52*, whole, the holder shortened ([capture](live-ended-960x540.png)).

### Scenario 4 (criterion 3): 960 × 540

1. The window is resized to 960 × 540; the agent runs the scenario, breaks
   the price and runs it again.

Expected: the status whole; the holder text shortened to *Claude Code ·
since HH:MM*; *Take back* at the right end; nothing above the bar moved.

Observed: as expected in Code: running
([capture](simulation-running-960x540.png)), passed
([capture](simulation-ended-960x540.png)), failed
([capture](failed-ended-960x540.png)). The holder reads *Claude Code ·
since 14:51*. Diagram and Split at 960 × 540, and the positions of the
explorer, the tabs and the header with and without a status, are checked
by the interaction test only (no input device to switch modes on this
host).

### Scenario 5 (criterion 4): the agent finishes

1. The agent calls `finish_editing`.

Expected: the window is the writer again; no bar, no status.

Observed: `finished: true`; the holder file names the window; the bar and
the status are gone ([capture](finished-960x540.png)). The server ended
with exit code 0 at the end of stdin.

### Against M4

| | M4 (round 4) | This branch |
| --- | --- | --- |
| Place | right of the bar, left of *Take back*, in place of the last change | the same |
| Colour and size | muted, meta size | the same |
| Running text | *Agent: running promotion-and-cart-price.a…* (cut) | *Agent: running promotion-and-cart-price.afs (simulation)…*, whole |
| Passed text | *Agent: promotion-and-cart-price.afs pass…* (cut) | *Agent: promotion-and-cart-price.afs passed · 18 steps · 14:52*, whole |
| Holder at 960 px | *Claude Code via apiforge mcp · since 10:42 · editing is paused here; …* | *Claude Code · since 14:51*, whole on hover: the ticket's fix of the round-4 cut |
| Holder at 1600 px | (not in M4) | the full text, as without a status |

The only differences are the ones the ticket asks for: the status is whole
and the holder text gives way. The harness design-review captures (an
`egui_kittest` aid, not QA evidence) agree:
[running at 960](harness-running-960x540.png),
[passed at 1600](harness-passed-1600x1000.png).

## Windows

Not run on Windows. The window's change is drawing and state; the server's
uses MCP-08b's channel and named pipe unchanged. The CI run of this branch
on `windows-2022` runs the acceptance tests above; its
`qa-captures-windows-2022` artifact shows the folder modes, not an
observing window. The ticket is not accepted for Windows on this report.

## Not verified

- Diagram and Split at 960 × 540 in the release binary: no input device to
  switch modes; the interaction test checks all three modes for the
  running, passed and failed lines.
- A cancelled run in the release binary: the script sent no
  `notifications/cancelled`; both criterion 1 tests cover it.
- The channel dropping during a run in the release binary (server killed):
  the criterion 4 test only.
- A window that connects to the server during a run: the channel test
  `a_window_that_connects_during_a_run_hears_that_it_runs` only, no window
  drawn.
- The hover text of the shortened holder: no pointer on this host.
- A component run alone, a run that fails before its first step (*failed*
  without a step), very long file names or labels (cut to 60 and 40
  characters): unit tests in `folder/holder.rs` only.
- Double scale; a screen reader.
- Mixed builds (a v0.1.0-rc.1 window with this server, or the reverse):
  only the decoding is tested (`apiforge-channel`: a `run` without the new
  fields reads).

## Regression smoke list

Not run in full: the change touches only the observing window's bar and
the server's channel messages. Item 10 (960 × 540) holds for the observing
window in Code (captures above); the bar tests of `mod mcp07` and
`mod mcp08b` pass unchanged.

## Verification

On `c2d356c`, all exit 0:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1 326 passed, 0 failed,
  36 ignored (the second run). The first run had one failure outside this
  change, `folder::tests::a_large_source_follows_the_caret_once_its_edit_is_analysed`
  (*storage did not become idle* within its 15 s, while a release build
  ran beside it on three cores); it passed three times alone and in the
  second full run.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
