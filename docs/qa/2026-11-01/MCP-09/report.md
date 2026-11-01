# MCP-09: The live toggle — QA report

- Branch: `feat/mcp-09-live-toggle`. Captures are from the release binaries
  of `f4c94bd` (`apiforge-desktop`; `apiforge` is unchanged since
  `eda3f06`).
- Built with: `cargo build --release --locked` (Rust 1.99.0).
- Compared against mock-up: M3,
  `docs/qa/2026-10-27/MCP-00/m3-*.png` (approved 2026-10-27 with the footer
  and *Apply settings* hidden while observing; *Cancel* kept by the owner's
  decision of 2026-10-28 on #93).
- Analysis model: Fable (MCP-00 revision 2). Implementation model: Claude
  Opus 5.5.
- Acceptance tests first: commit `f95782f`. Before the implementation all
  eight `mcp09_criterion_*` tests failed for the right reason: the dialog
  had no toggle, and a server started with `--allow-live` sent while the
  window's box was off. The implementation commit changed one helper of
  criterion 7's setup (a live run that a probe pre-empted is started again,
  as MCP-08b's `start_run` does); no assertion of an MCP-09 test changed.
  The same commit changed one assertion of an older interaction test,
  `a_read_only_window_offers_only_session_secret_values` (MCP-08a), from
  *Apply settings disabled* to *Apply settings absent*, the change this
  ticket's criterion 2 makes; the acceptance commit had already done so for
  MCP-08a's acceptance test.
- Verification: at the end of this report.

## Criteria and their evidence

| # | Criterion | Test (`folder/acceptance_tests.rs`, `mod mcp09`) | Release binary |
| --- | --- | --- | --- |
| 1 | Off for a project never set; ticked again after reopening; another project unaffected; kept by manifest id outside the project folder, which stays byte-identical | `mcp09_criterion_1_the_box_is_remembered_per_project_outside_the_folder`; unit tests in `consent.rs` | Scenario 1 (the file and its place) |
| 2 | Opens while observing; only the toggle, *Close* and *Cancel* enabled; footer note and *Apply settings* hidden; applies at once, nothing to save | `mcp09_criterion_2_settings_open_while_observing_and_the_box_applies_at_once` | Scenario 1 |
| 3 | Agent connected, box off: `live_disabled` with the settings message; ticking shows `allowed: true, source: "window setting"` within 1 s; the next live run sends | `mcp09_criterion_3_ticking_the_box_lets_the_next_live_run_send` | Scenario 1 |
| 4 | `--allow-live`, box off: refused while the window is connected; after it closes the flag applies | `mcp09_criterion_4_the_flag_neither_enables_nor_overrides_an_open_window` | not run (see *Not verified*) |
| 5 | No `--allow-live`, box on: sends while the window is connected | `mcp09_criterion_5_the_box_allows_a_server_started_without_the_flag` | Scenario 2 (the live run after ticking) |
| 6 | No tool, resource or server message changes the setting | `mcp09_criterion_6_no_tool_resource_or_message_changes_the_setting` | — |
| 7 | Unticking during a live run: it finishes with its cleanup, the dialog says so, the result is complete, the next live run is refused | `mcp09_criterion_7_unticking_during_a_live_run_lets_it_finish` | Scenario 2 |
| 8 | *Live allowed* in Code, Diagram and Split at 960 × 540 while ticked, never otherwise; clear of the mode tabs on a component's tab with *Publish* | `mcp09_criterion_8_the_badge_shows_in_every_mode_while_ticked` | Scenario 3 |
| 9 | QA against M3; verification passes | this report | — |

The server in these tests is a real process on real stdio, spawned as in
`mod mcp08b`, with the demo API on a free port. Two older tests change with
the ticket's behaviour: MCP-08a's settings test expects *Apply settings*
hidden while observing, and MCP-12's *permitted first live run* ticks the
window's box, since an open window now decides and `--allow-live` no longer
overrides it; MCP-12's *permission off* case is unchanged (`live_disabled`,
the window keeps the project).

## How the release binaries were driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
scratch configuration (`XDG_CONFIG_HOME`), data and runtime directories.
Clicks went through the X test extension (libXtst from the Ubuntu archive,
unpacked into a scratch directory, not installed); the screen was grabbed
from the display, and the 960 × 540 window is the same process resized
through Xlib. The release `apiforge mcp --folder …` (no `--allow-live`) was
driven over stdio by a scratch script (not committed) as the client *Claude
Code*. The project is the demo shop with its base URL pointed at a scratch
loopback API that answers every `GET` with `200 {"id": 123}` after six
seconds, so that a live run is in progress long enough to be seen.

## Linux

### Scenario 1 (criteria 1, 2, 3): the box in observer mode

1. Open the shop in the release window; start the agent; read
   `project_overview`.
2. The agent runs the shop scenario live while the window holds the project.
3. The agent creates `scenarios/live-check.afs` (a `GET` and a cleanup
   `GET`), so the clean window hands the project over; read the overview.
4. *Project → Project settings…*; tick the box; read the overview.

Expected: before the hand-over the overview says `no --allow-live`: no
window is connected to this server yet, and the overview never connects to
the one that holds the project (MCP-12), so it reports the flag. The live
run does connect to that window to hear its consent before asking for the
project, so it is refused with *live requests are off in this project's
settings* and the window keeps the project;
after the hand-over the overview says `off in the project's settings`; the
dialog opens while observing with only the toggle, *Close* and *Cancel*
enabled, no footer note and no *Apply settings*; ticking shows *On — …*,
the *Live allowed* badge, and the overview `window setting`.

Observed: as expected.

- Step 1: `holder: window`, `live_requests: {allowed: false, source: "no
  --allow-live"}`.
- Step 2: `live_disabled`, *live requests are off in this project's
  settings*, `details.source: "off in the project's settings"`, answered in
  0.62 s; the window stayed the writer, not observing
  [capture](refused-kept-1600x1000.png).
- Step 3: `wrote: true`; `holder: agent_session`, `live_requests:
  {allowed: false, source: "off in the project's settings"}`.
- Step 4: the dialog, off [1600 × 1000](settings-off-1600x1000.png),
  [960 × 540](settings-off-960x540.png); ticked
  [1600 × 1000](settings-on-1600x1000.png),
  [960 × 540](settings-on-960x540.png) with *On — the agent may send real
  requests to http://127.0.0.1:4391.* and *Observing · Live allowed* in the
  header. The overview read right after the click said `{allowed: true,
  source: "window setting"}`. `agent-consent.json` appeared in
  `$XDG_CONFIG_HOME/apiforge/` with `{"version": 1, "projects": {"shop":
  {"live": true, "changedAt": "…Z"}}}`; under the project folder only the
  agent's own files changed (`apiforge.project.json`,
  `.apiforge/diagram.json`, `scenarios/live-check.afs`, from step 3).

### Scenario 2 (criteria 5, 7): a live run, unticked while it runs

1. With the box ticked and the dialog open, the agent starts
   `run(scenarios/live-check.afs, live: true)`.
2. Two seconds later, untick the box.
3. Wait for the answer; read the dialog; run live again.

Expected: the run sends; the dialog says *A live run is in progress; it
will finish. No new one starts.*; the run finishes with its cleanup; the
line returns to *Off — the agent can only simulate.*; the next live run is
refused.

Observed: as expected at both sizes. The in-progress line
[1600 × 1000](settings-running-1600x1000.png),
[960 × 540](settings-running-960x540.png); the answer, about twelve seconds
after the start: `live: true`, `status: passed`, `ended_by: completed`,
steps `response` and `ok` (main) and `again` (cleanup), all `passed`; the
scratch API logged both requests. The dialog afterwards
[1600 × 1000](settings-after-run-1600x1000.png); the next live run
`live_disabled` with the settings message, and the overview
`{allowed: false, source: "off in the project's settings"}`.

### Scenario 3 (criterion 8): the badge

1. Tick the box again and close the dialog with *Cancel*.
2. At 960 × 540, Code, Diagram and Split; then open `components/product.afs`
   (a component's tab, with *Publish*).

Observed: *Observing · Live allowed* beside *Demo shop* in every mode, the
name in full: Code [960](header-code-960x540.png) and
[1600](header-code-1600x1000.png), [Diagram](header-diagram-960x540.png),
[Split](header-split-960x540.png); on the component's tab with *Publish* the
badges end about 20 px left of the *Code* tab
[capture](component-code-960x540.png). *Cancel* kept the ticked box.
Without the box (Scenario 1 before step 4) the header shows *Observing*
only.

The first build of this branch (`eda3f06`) reserved 80 px per badge, which
left the name 60 px wide at 960 × 540 with both badges (*Demo s…*), where
M3 shows *Demo shop*. `f4c94bd` reserves the badges' measured widths; the
captures above are from that build.

### Against M3

| | M3 | This branch |
| --- | --- | --- |
| Section *Agent* under the project name, separator above it | as drawn | the same |
| Toggle wording, check mark | *Allow the MCP agent to send live requests* | the same |
| Line when off / running | *Off — the agent can only simulate.* / *A live run is in progress; it will finish. No new one starts.* | the same |
| Line when on | *On — the agent may send real requests to the project's base URL.* | *On — the agent may send real requests to http://127.0.0.1:4391.*: the base URL itself, as the ticket's *What the owner will see* writes it (*… to <base URL>*); with an empty base URL the line reads as in M3 |
| Footer note and *Apply settings* while observing | drawn, then hidden by the owner's decision | hidden |
| *Cancel* | shown | shown and enabled (owner decision of 2026-10-28) |
| Badge | *Live allowed*, warning colour, after *Observing* | the same; hover text *An MCP agent may send live requests in this project. Change it in Project settings.* |
| Header at 960 with both badges | *Demo shop Observing Live allowed* | the same |
| Holder notice | M3's header capture shows MCP-08a's round-1 notice under the tabs | the bottom bar of M1 round 4 (MCP-00: M3 does not decide it) |

No other visible difference. The data contracts tab and the project name
are disabled, as in M3.

## Windows

Not run on Windows. The window's drawing is platform-independent; the
channel and its named pipe are MCP-08b's, with one more message on the
same connection. The CI run of this branch on `windows-2022` runs the
acceptance tests above. The ticket is not accepted for Windows on this
report.

## Not verified

- Criterion 4 in the release binary (closing the window while the agent
  holds the project, then a live run under `--allow-live`): covered by its
  test with the real server process; not driven here.
- The *Live allowed* badge with the window as the writer (not observing) in
  the release binary; the test covers both.
- A window of an older build, which says no consent when a connection
  opens: the server waits two seconds and refuses the live run, as for a
  window whose box is off. No test runs two builds.
- Two windows on one project with different consents: the newest one heard
  decides; no test.
- The design-review harness view `review_mcp09_live_toggle` was not
  rendered; the release captures above were compared with M3 instead.
- The changes of review round 1 (below) in the release binary: the
  once-a-second re-read of the consent file, the Ctrl+Enter guard, a silent
  connection counting as off, and the newer-version file rules. They were
  made after the captures and are covered by tests only (the Ctrl+Enter
  guard by `dialogs::tests::ctrl_enter_applies_only_with_an_apply_button`,
  added in round 3).
- Keyboard-only operation of the toggle and a screen reader session.
- Windows, beyond the CI test run.

## Review round 1

The automated review of #189 (cubic, 13 points) and the first Windows CI run
are answered on the pull request. Fixed: a connected window that has not
said its consent counts as off, never as the start flag, on the server's
slot and in a live run's question before the hand-over (new test
`mcp09_a_window_that_has_not_said_its_consent_is_off`); the window reads
`agent-consent.json` again once a second, so a change made in another
window is followed; a file written by a newer version is never overwritten
and a choice that cannot be written lasts for the window (unit tests in
`consent.rs`); Ctrl+Enter applies nothing in a dialog without an apply
button; criterion 1's folder snapshot lists the writer's lock file by its
length (Windows cannot read a held lock file); the ticket cites the owner's
decision of 2026-10-28 by date, without a link; ADR 0014, the development
guide and this report say the above. Nothing drawn changed; the captures
were not retaken. Verification of the round: `cargo fmt --all -- --check`;
`cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo
test --workspace --all-targets --locked`: 1 349 passed, 0 failed, 37
ignored; `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`;
all exit 0.

## Regression smoke list

Linux, 2026-11-01, release binaries of `f4c94bd`:

| # | Result |
| --- | --- |
| 2, 10 | `--screenshot` of the demo shop in Code, Diagram and Split at 960 × 540 and 1600 × 1000: all six captures at the requested size, the tree, the editor and the board shown, all controls visible (not committed) |
| 1, 3–9 | Not run on the release binary for this ticket; the workspace suite covers their tests |

The owner's Windows run is pending as for the earlier tickets.

## Verification

On `c8c3888` (review round 3, main with MCP-14 merged), all exit 0:
`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets
--locked -- -D warnings`; `cargo test --workspace --all-targets --locked`:
1 351 passed, 0 failed, 37 ignored; `cargo run -p apiforge-convert-v1
--locked -- --fixtures --check`. Earlier: `fa1660e` (round 2) 1 350 passed,
`57d199f` (round 1) 1 349, `9de6233` 1 346, `eda3f06` 1 343, each with 37
ignored. The captures are from `f4c94bd`; nothing drawn changed after it.
