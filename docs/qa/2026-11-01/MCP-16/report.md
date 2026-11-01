# MCP-16: The observing window follows the agent — QA report

- Branch: `docs/mcp-16-follow-agent`. Captures are from the release
  binaries of `3fa5afb` (the implementation `bf677d1` and a test-only
  clippy change).
- Built with: `cargo build --release --locked -p apiforge-desktop -p
  apiforge-cli` (Rust 1.99.0).
- Compared against mock-up: f1–f4 of [MCP-16](../../../tickets/MCP-16.md),
  approved by the owner on 2026-11-01, copied here from
  `mock/mcp-16-follow` at `f4bafb2`:
  [f1](f1-following-code-960x540.png), [f2](f2-unpinned-code-960x540.png),
  [f3](f3-following-diagram-1600x1000.png),
  [f4](f4-following-split-960x540.png).
- Analysis model: Fable. Implementation model: Claude Opus 5.5.
- Acceptance tests first: commit `e0e9276`. Before the implementation all
  fifteen `mcp16_criterion_*` tests and the three MCP-08a tests that now
  unpin first failed for the right reason: the bar had no *Follow the
  agent* toggle, or the window never went to the agent's document. After
  that commit no assertion changed. Criterion 2's last edit changes a
  label instead of a timeout (the server refused the timeout, which
  disagreed with the statement's `@config`); the review view waits for the
  agent's save before it captures; three tab-list comparisons are written
  with a slice, as clippy asks; and the tests wait until the window has
  followed the hand-over's commit before the agent writes again (below).

## Review of the first CI run

The first CI run of this branch (`3fa5afb`, Linux) failed criterion 7:
*the window never showed product.afs saved*. A test steps the window only
while it waits, so the hand-over's commit and the agent's next write were
sometimes read in one check, and the bar named the first change of both
(*promotion-and-cart-price.afs saved and 1 more*). The window does not
know which of two commits read together came last; locally the same
folding had sent criteria 4, 6, 11 and 12 to the scenario instead of the
agent's second document, which the implementation now handles by
preferring a document other than the active one (ticket, implementation
notes). The tests now wait for the hand-over's commit to be followed
(`observe` in `mod mcp16`), as a person's window, drawn continuously,
does; no assertion changed. With it the `mcp16`, `mcp10` and `mcp08a`
tests passed three times in a row, four at a time.

## Criteria and their evidence

| # | Criterion | Test (`folder/acceptance_tests.rs`, `mod mcp16`) | Release binary |
| --- | --- | --- | --- |
| 1 | *Follow* pressed when observing starts: hand-over, and a window opened while the agent holds | `mcp16_criterion_1_following_is_on_when_observing_starts` | Scenarios 1 and 6 |
| 2 | A saved document is followed within 2 s in the person's view; first changed line in view and marked; camera and selection kept in Diagram; marks go with the next commit | `mcp16_criterion_2_a_saved_document_is_followed_in_{code,diagram,split}` | Scenarios 1 (Code, Split) and 2 (Diagram) |
| 3 | A created document is open and active at its top, no marks | `mcp16_criterion_3_a_created_document_is_followed` | Scenario 4 |
| 4 | A published component's tab comes to the front, scroll unchanged | `mcp16_criterion_4_a_publication_is_followed` | Scenario 4 |
| 5 | A deletion closes the tab with MCP-08a's notice, the tab closing chose stays active; settings alone move nothing | `mcp16_criterion_5_delete_and_settings_move_nothing` (the holder is a session in the test's process: no MCP tool deletes or edits settings) | not run |
| 6 | A run moves no tab and no scroll | `mcp16_criterion_6_a_run_moves_nothing` | Scenarios 1 and 2 |
| 7 | Unpinned by the toggle the place is kept; the bar names the change | `mcp16_criterion_7_unpinned_keeps_the_place`; MCP-08a's criteria 2–4 tests unpin first and pass | Scenario 3 |
| 8 | Tab click, explorer, Ctrl+Tab, closing the followed tab unpin; scroll, zoom, block selection, mode do not; *Follow* goes to the last change at once | `mcp16_criterion_8_navigation_unpins_and_follow_pins` | Scenario 3 (the toggle both ways) |
| 9 | Named *Follow the agent* with its state, reached by Tab, hover texts | `mcp16_criterion_9_the_toggle_is_accessible` | Scenario 3 (hover texts) |
| 10 | At 960 × 540 in every mode with MCP-10's status: whole, disjoint, inside; the holder gives way | `mcp16_criterion_10_the_bar_fits_at_the_minimum` (running, passed and the last change, each in Code, Diagram, Split) | Scenario 1 (Code, Split) |
| 11 | Followed tabs stay after the agent finishes and after *Take back*; editable | `mcp16_criterion_11_followed_tabs_stay` | Scenario 5 (`finish_editing`) |
| 12 | The next session follows again after an unpin | `mcp16_criterion_12_following_resets_per_observation` | Scenario 6 |
| 13 | QA against the mock-up in all four states; verification passes | this report | — |

The server in these tests is a real process on real stdio, spawned as in
`mod mcp08b`. Unit tests: the changed lines of a commit
(`apiforge-project`, `external.rs`), line offsets and the order of a
commit's changes (`folder/follow.rs`).

## How the release binaries were driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
no window manager, scratch configuration, data and runtime directories.
Clicks and pointer moves went through the X test extension (libXtst from
the Ubuntu archive, unpacked into a scratch directory, not installed); the
screen was grabbed from the display, and the window was resized through
Xlib. The release `apiforge mcp --folder …` was driven over stdio by a
scratch script (not committed) as the client *Claude Code*, with the test
seam `APIFORGE_MCP_TEST_STEP_DELAY_MS=400` so that a run lasts long
enough to be captured. The project is the demo shop as `prepare_project`
writes it, with no local state. Captures were taken 1.5 s after each tool
call returned.

## Linux

### Scenario 1 (criteria 1, 2, 6, 10; f1 and f4): following in Code and Split

1. Open the shop in the release window at 960 × 540 (Code); open
   *product.afs* from the explorer, so the scenario's tab is not active
   ([capture](writer-on-product-960x540.png)).
2. The agent writes the scenario with lines 12–14 changed; the clean
   window hands the project over.
3. The agent runs the scenario; meanwhile switch to Split.

Expected: the window goes to the scenario's tab in Code; lines 12–14
carry the accent bar; *Follow* pressed; then MCP-10's running status, the
holder in its short form; Split keeps the code with its marks.

Observed: as expected. After the write the scenario's tab is active with
lines 12–14 marked, the last change *23:28 · promotion-and-cart-price.afs
saved*, *Follow* pressed ([capture](followed-code-960x540.png)). The lines
were in view, so the code did not scroll. While the run goes: *Claude
Code · since 23:28*, *Agent: running promotion-and-cart-price.afs
(simulation)…*, *Follow*, *Take back*, all whole
([f1](release-f1-following-code-960x540.png)); in Split the same bar and
marks, the board beside ([f4](release-f4-following-split-960x540.png)).
The run moved nothing.

### Scenario 2 (criteria 2, 6; f3): Diagram at 1600 × 1000

1. Resize to 1600 × 1000; switch to Diagram.
2. The agent runs the scenario to its end.

Expected: the board as before, the full holder text, *Agent: … passed ·
18 steps · HH:MM*, *Follow* pressed.

Observed: as expected ([f3](release-f3-following-diagram-1600x1000.png)):
*Agent: promotion-and-cart-price.afs passed · 18 steps · 23:29*; the
board shows the agent's labels (*Promotional total*, *Field present*).

### Scenario 3 (criteria 7, 8, 9; f2): unpinned, then pinned again

1. Resize to 960 × 540; Code. Click *Follow*; click the *product.afs*
   tab.
2. The agent writes the scenario again (a comment under its first line).
3. Hover *Follow*; click it; hover it again.

Expected: released, *Follow* looks like *Take back*; the window stays on
the component; the bar names the change; the full holder text is cut
with "…"; the released hover text. Pressing *Follow* goes to the
scenario at once with the new line marked; the pressed hover text.

Observed: as expected. Unpinned on *product.afs*: *Claude Code via
apiforge mcp · since 23:28 · editing is paused here; this window follo…*,
*23:29 · promotion-and-cart-price.afs saved*, *Follow* released
([f2](release-f2-unpinned-code-960x540.png)); hover: *Click to follow the
agent again* ([capture](follow-hover-released-960x540.png)). Pressed
again: the scenario active, line 2 marked beside the warning's dot in the
gutter, the text not moved ([capture](repinned-960x540.png)); hover:
*Follows the agent to every file it changes; click to stay where you
are* ([capture](follow-hover-pressed-960x540.png)).

### Scenario 4 (criteria 3, 4): a created document and a publication

1. The agent creates `scenarios/added-by-agent.afs`.
2. The agent publishes `components/product.afs` (revision 2).

Expected: the new document's tab opens beside the active one, active, at
its top, no marks; then the component's tab comes to the front.

Observed: as expected ([created](created-960x540.png): the tab right of
the scenario's, *23:30 · added-by-agent.afs added*;
[published](published-960x540.png): *product.afs* active, *23:30 ·
product.afs published*). The pressed hover text still shows in both: the
pointer stayed on *Follow* after step 3.

### Scenario 5 (criterion 11): the agent finishes

1. The agent calls `finish_editing`.

Expected: the window is the writer again; the tabs following opened stay.

Observed: `finished: true`; the holder file names the window; the bar is
gone and the three tabs stay ([capture](finished-960x540.png)).

### Scenario 6 (criteria 1, 12): the next session

1. The agent writes the scenario once more, while the person is on
   *product.afs*.

Expected: the window hands the project over and follows again: the
scenario's tab, the new line marked, *Follow* pressed.

Observed: as expected ([capture](second-session-960x540.png)); of two
identical comment lines the second is marked, line 3, the first line
where the texts differ. The hover box over the code is from the parked
pointer.

### Against the mock-up

| State | Mock-up | Release binary | Difference |
| --- | --- | --- | --- |
| f1 · Code 960 × 540, following | [f1](f1-following-code-960x540.png) | [release](release-f1-following-code-960x540.png) | none in layout; lines 12–14 hold the agent's real change, the mock-up's were the demo's text |
| f2 · Code 960 × 540, unpinned | [f2](f2-unpinned-code-960x540.png) | [release](release-f2-unpinned-code-960x540.png) | none in layout |
| f3 · Diagram 1600 × 1000, following | [f3](f3-following-diagram-1600x1000.png) | [release](release-f3-following-diagram-1600x1000.png) | none in layout; two blocks carry the agent's new labels |
| f4 · Split 960 × 540, following | [f4](f4-following-split-960x540.png) | [release](release-f4-following-split-960x540.png) | none in layout |

The approved visual choices hold: *Follow* pressed has the soft accent
fill, the accent border and the accent pin, and released it looks like
*Take back*; 8 px between *Follow* and *Take back* and 16 px before the
status, placed as the mock-up's code placed them; the changed lines' 3 px
bar sits at the gutter's right edge, clear of the diagnostic's dot and
the number, and the text does not move; at 960 px the full holder text
is cut with "…". The mock-up's status was a placeholder; the release
shows MCP-10's real status with the same wording. The harness draws the
window 8 px inside its capture, so the release's controls sit 8 px up
and left of the mock-up's.

## Windows

Not run on Windows. The change is drawing and state in the window and a
diff in `apiforge-project`; the channel and the named pipe are unchanged.
The CI run of this branch on `windows-2022` runs the acceptance tests
above; its `qa-captures-windows-2022` artifact shows the folder modes, not
an observing window. The ticket is not accepted for Windows on this
report.

## Not verified

- Criterion 5 (a deletion, the settings alone) in the release binary: the
  MCP server has no tool that deletes a document or edits the settings;
  the test drives a holder session in its own process.
- Auto-unpin by a tab click, the explorer, Ctrl+Tab and closing the
  followed tab, and Tab and Space on the toggle, in the release binary:
  scenario 3 used the toggle and clicked a tab only after unpinning. The
  criterion 8 and 9 tests cover them.
- Diagram's camera and selected block across a followed commit in the
  release binary: the criterion 2 Diagram test only.
- A changed line out of view (the code scrolls and centres it): the
  criterion 2 tests only; in the release every changed line was in view.
- Diagram at 960 × 540 and Split at 1600 × 1000 in the release binary:
  the criterion 10 test checks all three modes at 960 × 540.
- The 2 s bound in the release binary: captures were taken 1.5 s after
  each call. In the tests, one local run under load (three cores, another
  build alongside) measured 2.12 s for criterion 11; the repeated runs
  passed.
- Several commits folded into one reload (the window then prefers a
  document other than the active one): reasoned in the ticket's
  implementation notes, exercised only incidentally by tests that write
  twice in quick succession.
- Double scale; a screen reader; mixed builds (no protocol change).

## Regression smoke list

Not run in full: the change touches the observing window's bar, its
navigation after a followed commit and the code's gutter. Item 10
(960 × 540) holds for the observing window in Code and Split (captures
above). Items 1 and 2 held in this session (the release window opened the
shop as its writer; the explorer and Code worked). The whole desktop suite
passes (below), the bar tests of MCP-07, MCP-08a/b/c and MCP-10 included.

## Verification

On `3fa5afb`, all exit 0:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked --no-fail-fast`: 1 378
  passed, 0 failed, 40 ignored
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

The later commits change only `mod mcp16`'s setup and documents; on them
fmt and clippy were run again and the `mcp16`, `mcp10` and `mcp08a` tests
passed (see above), and CI runs the whole suite on both platforms.
