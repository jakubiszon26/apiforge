# UX-02: Run mode is a setting; the Run button reads *Run simulation* or *Run live* — QA report

- Branch: `docs/ux-02-run-mode` (#244). The captures come from the release
  binaries of `a3aa0b9`: `apiforge-desktop`, and `apiforge` for the agent
  in scenario 4.
- Built with: `cargo build --release --locked` (Rust 1.99.0).
- Compared against the mock-up: UX-00 (`mock/ux-00` at `2c1fa79`), captures
  `docs/qa/2026-11-02/UX-00/ux02-*`. The owner approved it as a whole on
  2026-11-02: the violet from `ux02-run-simulation-violet-960x540`, the
  chevron from `ux02-run-simulation-chevron-960x540` and
  `ux02-run-options-chevron-960x540`, and 8 px more left margin on
  *Simulation* and *Live*.
- Analysis model: Fable. Implementation model: Claude Opus 5.5.
- Acceptance tests first: commit `abcce1d`. Before the implementation, all
  seventeen tests of `mod ux02` and `run_mode::tests` failed for the right
  reason:
  - the header had *Run* and the text button *Run options*;
  - there was no chevron and no *Simulation* or *Live* item;
  - the button and the badge were not violet;
  - the stub store forgot every choice.

  The implementation (`f033e68`) changed none of their assertions. One of
  those unit tests needed a lint fix, a `format!` with no arguments, to
  pass clippy.
- Verification: at the end of this report.

## Criteria and their evidence

Tests are in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`,
`mod ux02`, unless the table says otherwise.

| # | Criterion | Test | Release binary |
| --- | --- | --- | --- |
| 1 | *Run simulation* for a project never set, *Run live* once *Live* is chosen; *Run simulation* filled with `theme::SIMULATION` (112 · 76 · 182) and the SIMULATION badge in the same constant; *Run live* in the accent | `ux02_criterion_1_the_button_reads_its_mode`, `ux02_criterion_1_simulation_is_one_violet` | Scenarios 1, 2 (colour sampled from the captures) |
| 2 | The chevron *Run options* at the button's right opens the menu; no *Run live requests*; *Simulation* and *Live* at least 8 px right of RUN MODE; the name whole at 960 × 540 | `ux02_criterion_2_the_chevron_opens_the_menu`, `ux02_criterion_2_the_mode_items_are_indented`, `ux02_criterion_2_the_project_name_is_whole_at_960` | Scenarios 1, 2 |
| 3 | Kept per person and project in `apiforge/run-mode.json` by manifest id; a second window starts with it, another project in Simulation; the project folder byte-identical | `ux02_criterion_3_the_mode_is_kept_per_project_and_the_project_is_untouched`; unit tests in `run_mode.rs` | Scenario 3 |
| 4 | The button runs the mode; while observing it reads and runs *Run simulation*, *Live* is disabled with MCP-08a's hover, *Run live* is back after *Take back* | `ux02_criterion_4_the_button_runs_the_chosen_mode`, `ux02_criterion_4_observing_runs_a_simulation`; the journal and invalid-code prompts by their existing tests | Scenarios 1, 2, 4 |
| 5 | *Live allowed* unchanged beside *Run live* | `ux02_criterion_5_live_allowed_is_unchanged`; `mod mcp09` passes unchanged | Scenario 2 |
| 6 | Matches `ux02-*` at both sizes; verification passes | this report | — |
| 7 | The header fits in Code, Diagram and Split at both sizes, either mode, scenario and component tabs, with *Live allowed* | `ux02_criterion_7_the_header_fits` | Scenario 2 (component tab) |
| 8 | A text file's tab or no tab keeps *Runs* → *Review legacy runs* | `ux02_criterion_8_the_runs_menu_is_unchanged` | not driven |
| 9 | Tab reaches *Run simulation*, then *Run options*; the chevron's name and its menu from the keyboard | `ux02_criterion_9_the_controls_are_accessible` | not driven |

Existing tests that clicked *Run* or *Run live requests* now click the new
labels; there are about twenty such call sites. Three assertions change
with the ticket:
- a component's header reads *Publish*, *Run simulation*, *Run options*,
  in that order;
- the *Run* hover text names the sample responses;
- MCP-08b's *Take back* ends where the run controls now end, at the
  chevron.

The long-name header test no longer looks for painted text on the chevron,
which has none; criterion 7 checks the chevron's position instead.

## How the release binaries were driven

The run was on Linux, under Xvfb at 1920 × 1200 with Mesa's Lavapipe
Vulkan driver and the X11 backend. Configuration, data and runtime
directories were scratch copies (`XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
`XDG_RUNTIME_DIR`).
- Clicks, keys and hovers went through the X test extension, using libXtst
  from the Ubuntu archive, unpacked into a scratch directory and not
  installed.
- The screen was grabbed from the display. The 960 × 540 window is the same
  process, resized through Xlib.
- The project is the demo shop as `prepare_project` writes it, with its
  base URL pointed at a scratch loopback API on port 4392. That API answers
  every request with `200` and logs the request line, so the log shows
  whether a run sent anything.
- MCP-09's consent for the project was written to `agent-consent.json`
  before the start, so *Live allowed* shares the header, as in every UX-00
  capture. No `run-mode.json` existed before the start.
- Scenario 4's agent is the release `apiforge mcp --folder …`, driven over
  stdio as the client *Claude Code* by a scratch script (not committed).

## Linux

### Scenario 1 (criteria 1, 2, 4): a project never set runs a simulation

1. Open the shop; switch to Diagram.
2. Click *Run simulation*.
3. Click the chevron.

Expected:
- The button reads *Run simulation* in violet, with the chevron 2 px to its
  right.
- The run reports SIMULATION in the same violet, and the API receives
  nothing.
- The menu holds RUN MODE, *Simulation* (selected) and *Live* with their
  hints, the line, a separator and *Review legacy runs*, and no *Run live
  requests*.

Observed: as expected at both sizes.
- The run: [1600 × 1000](run-simulation-1600x1000.png),
  [960 × 540](run-simulation-960x540.png). The run passed, 8 steps, and the
  API log stayed empty.
- The menu: [1600 × 1000](run-options-simulation-1600x1000.png),
  [960 × 540](run-options-simulation-960x540.png).
- Measured on the 960 × 540 captures:
  - The button's fill is exactly `112 · 76 · 182`, and so is the darkest
    pixel of the SIMULATION badge's text.
  - The chevron's icon is centred 17.5 px right of the button's edge (a
    32 px button 2 px away).
  - The RUN MODE caption's first glyph is at x = 647. *Simulation*'s is at
    659 and *Live*'s at 657, so they sit 12 px and 10 px in from the
    caption. In `ux02-run-options-chevron-960x540` the same gaps are 4 px
    and 2 px, an 8 px gain. The selection highlight starts at 654.

### Scenario 2 (criteria 1, 2, 4, 5, 7): *Live* is chosen and runs live

1. In the menu, click *Live*.
2. Click *Run live*.
3. Open the menu again.
4. At 960 × 540, open `components/product.afs` from *Files*.

Expected:
- *Run live* in the accent beside *Live allowed*.
- The run reports LIVE API and the API receives the requests.
- *Live* is selected in the menu.
- On a component's tab, *Publish*, *Run simulation* or *Run live*, and the
  chevron fit beside the name and the badge.

Observed: as expected.
- The button is filled with `224 · 89 · 39`, the accent.
- `run-mode.json` appeared at the click (see scenario 3).
- The live run: [1600 × 1000](run-live-1600x1000.png),
  [960 × 540](run-live-960x540.png). The API logged `GET /products/123`. The
  run *failed* at *Active promotion*: the scratch API's body is not the
  product the component expects. The mode is what this scenario checks, not
  the scenario's result.
- The menu with *Live*: [1600 × 1000](run-options-live-1600x1000.png),
  [960 × 540](run-options-live-960x540.png).
- The component's tab: [960 × 540](component-tab-960x540.png) with *Run
  simulation*, and [1600 × 1000](component-tab-live-1600x1000.png) with
  *Run live* (scenario 3's restart). *Demo shop* and *Live allowed* are
  whole in every capture, and the badge keeps its colour and text.

### Scenario 3 (criterion 3): the mode is kept, the project is untouched

1. Choose *Live* (on the component's tab) and quit.
2. Read the configuration directory and compare the project folder.
3. Start the window again on the same project and configuration.

Expected: `run-mode.json` beside `agent-consent.json` names `shop` with
`live`; the project's files are unchanged; the new window reads *Run live*.

Observed: as expected.
- `$XDG_CONFIG_HOME/apiforge/run-mode.json` holds
  `{"version": 1, "projects": {"shop": {"mode": "live", "changedAt":
  "2026-11-02T20:09:43Z"}}}`, beside the untouched `agent-consent.json`.
- The SHA-256 of every file under the project folder is the same before
  the first start and after the quit. The window's machine-local
  `.apiforge/local` is excluded: workspace state, the holder and the
  last-valid record.
- The restart opens on the component's tab with *Run live*
  ([capture](component-tab-live-1600x1000.png)).

### Scenario 4 (criterion 4): observing runs a simulation

1. With *Live* stored, open the scenario's tab.
2. Start the agent and let it write the scenario. The clean window hands
   the project over.
3. Open the menu and hover *Live*.
4. Close the menu and click *Run simulation*.
5. Click *Take back*.

Expected:
- While the window observes, the button reads *Run simulation* in violet.
- *Live* is disabled with *Live requests are not sent from this window
  while another process holds the project.*
- The run is a simulation and sends nothing.
- `run-mode.json` still says `live`.
- After the take-back, *Run live*.

Observed: as expected.
- Observing: [header](observing-1600x1000.png), with *Observing · Live
  allowed* and the holder bar.
- The menu, with *Simulation* shown selected (the mode the button runs),
  *Live* greyed out and the hover text:
  [capture](observing-options-1600x1000.png).
- The simulation passed and the API log stayed empty
  ([capture](observing-run-1600x1000.png)). `run-mode.json` still read
  `live`.
- After *Take back*: [*Run live*](taken-back-1600x1000.png).

### Against the mock-up

| | UX-00 (`ux02-*`) | This branch |
| --- | --- | --- |
| *Run simulation* | violet `112 · 76 · 182`, white text (`-violet-`) | the same, sampled from the release capture |
| SIMULATION badge | blue in every capture (decided: takes the violet) | violet `112 · 76 · 182` |
| *Run live* | accent, beside *Live allowed* (`ux02-run-live-*`) | the same |
| *Run options* | chevron 2 px right of the button (`-chevron-`) | the same; at 960 × 540 the harness render of this branch puts the button at x = 767–901, the same pixels as `ux02-run-simulation-chevron-960x540` |
| Menu | RUN MODE, *Simulation*, hint, *Live*, hint, separator, the line, separator, *Review legacy runs* | the same order and words; *Simulation* and *Live* 8 px further in (the owner's addition); their hints stay at the caption's edge, as the ticket specifies |
| Name at 960 × 540 | *Demo shop* whole (`-chevron-`) | whole, also on a component's tab with *Publish* |
| Component's tab | *Publish*, *Run simulation*, chevron (`ux02-component-tab-960x540`) | the same |

Other visible differences:
- The release window's content starts 8 px further right than the harness
  renders, at both sizes. *Project* is at x = 16 rather than 24, and the
  run controls end 8 px nearer the window's edge. The design-review
  harness of this branch matches the mock-up's pixels. The difference is
  between the native frame and the harness, and it was there before this
  ticket.
- The release captures show the band *Running saved code.* under the tabs.
  The mock-up's review view had cleared it. It is today's notice, which
  UX-04 moves into the header.
- A violet button under the pointer darkens by the accent's own ratios: 8 %
  on hover and 17 % while pressed. The ticket's appendix proposed the mock
  helper's `gamma_multiply(0.9)`, which lightens an opaque fill on a white
  header. The UX-00 captures show no hovered button, so nothing approved
  is changed.
- The title's reserved width counts the wider mode label whatever the mode.
  The mock-up measured the current label. The name therefore keeps one
  width when the mode changes; the mode tabs move with the button's width.

## Windows

The CI run of `f033e68` (`41512166921`, the same code as `a3aa0b9` but for
an import's place) ran the whole suite, `mod ux02` included, on
`windows-2022`, and passed. It uploaded `qa-captures-windows-2022`. That is
the migrated demo shop, named *Sklep demonstracyjny*, at its default mode
of Simulation:
- [Diagram at 960 × 540](windows-diagram-960x540.png) and
  [Code at 960 × 540](windows-code-960x540.png) show *Run simulation* in
  violet with the chevron at its right. The longer project name is whole.
- The run's 1600 × 1000 captures are 1024 × 768, which the run's warnings
  note (the runner's display is smaller), and they are not cited.

The menu, the live run, the stored mode and observing were not driven on
Windows; their tests passed there. The ticket is not accepted for Windows on
this report.

## Not verified

- Criteria 8 and 9 in the release binary: *Runs* on a text file's tab or
  with no tab, and the keyboard path. Both are covered by their tests only.
- The two prompts on *Run live*, a journal that is unavailable and invalid
  code (*Run this version*), in the release binary. Their existing tests
  pass, re-pointed at the new labels.
- The hover and pressed shades of the violet button: no capture was taken.
- A screen reader session.
- Two windows on two projects changing their modes at the same moment. The
  file is read again before each write, so each keeps its own entry, but
  two writes in the same instant can lose one, as with the consent file. No
  test covers it.
- `review_ux02_run_mode` was rendered once during the work, at both sizes,
  and its captures were compared with the mock-up; they are not committed.

## Regression smoke list

Linux, 2026-11-02, release binaries of `a3aa0b9`:

| # | Result |
| --- | --- |
| 2 | The shop opened by `--folder`: the explorer shows the tree, and the scenario opens in Code (scenario 1's first frame) |
| 6 | Pass: *Run simulation* reports every step, and the API log stayed empty (scenarios 1, 4) |
| 10 | Pass: Diagram at 960 × 540 in scenarios 1 and 2 shows every control. CI's `qa-captures-ubuntu-24.04` of `f033e68` has all six standard captures, Code, Diagram and Split at both sizes, each with *Run simulation* at the same place (x = 775–909 at 960, 1415–1549 at 1600) |
| 1, 3–5, 7–9 | Not run on the release binary for this ticket. The workspace suite covers their tests |

The owner's Windows run is pending, as for the earlier tickets.

## Verification

On `f033e68`, all exited 0:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1 439 passed, 0 failed,
  44 ignored
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

`a3aa0b9` moves one `use` line. It passes `cargo fmt --all -- --check`
and `cargo clippy --workspace --all-targets --locked -- -D warnings`, and
its release build is the one captured above. CI on `f033e68` passed on
`ubuntu-24.04` and `windows-2022`.
