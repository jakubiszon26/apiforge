# MCP-08c: The hand-over prompt when the window has unsaved work — QA report

- Branch: `feat/mcp-08c-unsaved-prompt`. Captures are from the release
  binaries of `d245c9c`.
- Built with: `cargo build --release --locked` (Rust 1.99.0).
- Compared against mock-up: M2,
  `docs/qa/2026-10-27/MCP-00/m2-handover-prompt-*.png` (approved 2026-10-27).
- Analysis model: Fable (MCP-00 revision 2). Implementation model: Claude
  Opus 5.5.
- Verification on `d245c9c`, all exit 0:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --all-targets --locked`: 1 316 passed,
    0 failed, 34 ignored
  - `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
- Acceptance tests first: commit `c2e9931`. Before the implementation all
  six `mcp08c_criterion_*` tests and the two adapted MCP-08b refusal tests
  failed for the right reason: *the window never asked*. The implementation
  commit changed one helper of the tests' setup (the component is now found
  by its path: the first component of the demo is not the product); no
  assertion changed.

## Criteria and their evidence

| # | Criterion | Test (`folder/acceptance_tests.rs`, `mod mcp08c`) | Release binary |
| --- | --- | --- | --- |
| 1 | Prompt lists two documents and the name; the agent waits 30 s, `handover_pending`, nothing written, the prompt stays | `mcp08c_criterion_1_the_prompt_lists_the_work_and_the_agent_hears_pending` | Scenario 1, the two documents only; the dirty name is harness-only (recovery keeps no unsaved settings) |
| 2 | *Save all and hand over* saves every item, the window observes, the next call succeeds; a failed save keeps the project, shows the error, refuses with the reason | `mcp08c_criterion_2_save_all_saves_everything_and_hands_over`, `mcp08c_criterion_2_a_failed_save_keeps_the_project_and_refuses_with_the_reason` | not run (needs a click) |
| 3 | *Not now*: nothing saved, the writer stays, the pending or next call is refused with *the person chose to keep editing*; after Ctrl+S the call is granted | `mcp08c_criterion_3_not_now_keeps_editing_and_ctrl_s_lets_the_next_call_in` | not run (needs a click) |
| 4 | A held recovery draft is named; the prompt cannot decide it | `mcp08c_criterion_4_a_held_draft_is_named_and_not_decided_in_the_prompt` | not run |
| 5 | Inside the window at 960 × 540; Escape is *Not now* | `mcp08c_criterion_5_the_prompt_fits_960_by_540_and_escape_is_not_now` | Scenario 1 (fit only) |
| 6 | QA against M2; verification passes | this report | — |

The server in these tests is a real process on real stdio, spawned as in
`mod mcp08b`. The MCP-08b tests whose criterion this ticket changes
(`unsaved_work_refuses_the_hand_over_and_keeps_the_text`,
`unsaved_work_refuses_a_publication_and_nothing_is_published`) now answer
the prompt with *Not now*; they still check that nothing is written, the
text is untouched and the bar says *An agent asked to edit this project;
save your work first*.

## How the release binaries were driven

Linux, Xvfb 1920 × 1200 with Mesa's Lavapipe Vulkan driver, X11 backend,
scratch configuration, data and runtime directories, no window manager and
no input device: the window was never clicked or typed into. The release
`apiforge mcp --folder …` was driven over stdio by a scratch script (not
committed) as the client *Claude Code*: `initialize`, `read_source`,
`write_source`. The window's screen was grabbed from the display; the
960 × 540 window is the same process resized through Xlib. The unsaved work
is the demo shop's own local recovery: unsaved text in
`scenarios/promotion-and-cart-price.afs` and `components/product.afs`,
written by a scratch test (not committed) as a window closed with unsaved
work leaves it; the release window restores it on opening.

## Linux

### Scenario 1 (criteria 1 and 5): the agent asks, the person does not answer

1. Open the shop in the release window; the recovery restores the two
   documents' unsaved text.
2. The agent writes the scenario with a comment added, from the version it
   read.
3. Grab the window 4 s after the call started, and again after its answer.
4. Repeat 2–3 with the window resized to 960 × 540.

Expected: the prompt *An agent asks to edit this project*, naming *Claude
Code via apiforge mcp* and listing both documents, with *Save all and hand
over* and *Not now*; the call answers `handover_pending` after about 30 s
with `wrote: false`; nothing on disk changes; the prompt stays.

Observed: as expected at both sizes. The prompt lists
*scenarios/promotion-and-cart-price.afs — unsaved code and layout* and
*components/product.afs — unsaved code and layout* (a restored draft
carries its unsaved layout too, so both are named)
[1600 × 1000](prompt-1600x1000.png), [960 × 540](prompt-960x540.png); before
the request: [capture](before-1600x1000.png). Each call answered after
30.4 s with `code: handover_pending`, `wrote: false`, the window as holder;
the scenario on disk had no agent text; the holder file still named the
window. The grabs after each answer are byte-identical to the ones above:
the prompt stayed. The second request (at 960 × 540) found the prompt
already open and waited on it. The server's stderr has *the window has not
answered yet* once per call.

At 960 × 540 the whole prompt is inside the window with room to spare;
criterion 5's test also checks the heading, the three items and both
buttons against the window's rectangle at 960 × 540 with the project name
dirty as well.

### Against M2

| | M2 | This branch |
| --- | --- | --- |
| Heading, sentence, wording of the list and the buttons | as listed | the same |
| Position | centred | centred |
| Buttons | *Save all and hand over* (primary, check icon) left of *Not now*, right-aligned | the same |
| Bullets | indented, `•` | the same |
| Third item *Project settings — name* | shown | shown in the harness captures [960](harness-prompt-960x540.png), [1600](harness-prompt-1600x1000.png); not in the release captures, whose unsaved work comes from recovery, which keeps no unsaved settings |
| Item wording when a layout is unsaved too | (not shown) | *unsaved code and layout* |

No visible deviation from M2 beyond the data shown. The harness captures
(an `egui_kittest` design-review aid, not QA evidence) use the same three
items as M2 and match it line for line; their header shows *Demo shop,
renamed* and *Save* enabled because the name and the texts are unsaved,
which M2's mock data did not show.

### Scenarios 2 to 4 (criteria 2, 3 and 4): harness only

They need a click or a key in the window, which this host cannot give the
release binary. The tests above drive them through the window's named
widgets, with the real server process:

- *Save all and hand over*: the scenario and the product on disk are the
  person's texts byte for byte, `apiforge.project.json` carries the new
  name, the window observes and names *Claude Code via apiforge mcp*; the
  agent's call that was waiting was based on the old version and does not
  overwrite the person's text; its next `write_source` answers
  `wrote: true` and the holder is the agent.
- The failing save (the storage's fault seam, failing the scenario's
  replace on the window's storage worker): the window stays the writer and
  the holder; the error notice names *promotion-and-cart-price.afs*; the
  agent's call answers `handover_refused` with the reason *the window could
  not save the person's work* (no path, no error text crosses the channel);
  the scenario on disk and in the editor are unchanged.
- *Not now*: the waiting call is `handover_refused` with *the person chose
  to keep editing*; disk unchanged; the bar says *An agent asked to edit
  this project; save your work first*. A call whose agent went away while
  the prompt was open leaves the prompt open; *Not now* then answers the
  next call at once with the same reason. After Ctrl+S in the editor the
  next call answers `wrote: true` and the window observes.
- A held draft (unsaved work of an earlier session against a file changed
  since): the prompt lists *scenarios/promotion-and-cart-price.afs —
  unsaved work from an earlier session, to keep or discard*; the prompt's
  only buttons are *Save all and hand over* and *Not now*; the draft is
  still held after *Not now*.

## Windows

Not run on Windows. The change is in the window's own drawing and state;
the channel and the named pipe are MCP-08b's, unchanged. The CI run of this
branch on `windows-2022` runs the acceptance tests above; its
`qa-captures-windows-2022` artifact shows the folder modes, not the prompt.
The ticket is not accepted for Windows on this report.

## Not verified

- Clicking *Save all and hand over* or *Not now*, and pressing Escape, in
  the release binary: no input on this host; covered by the harness tests.
- A held draft, unfinished inspector fields, unsaved text files and
  *environment* or *contracts* settings in the release binary: harness
  (held draft) or not at all (the others have no test; they share the code
  path of the listed items).
- *Save all and hand over* with work a save cannot settle, in the release
  binary. In the harness, `save_all_with_a_held_draft_saves_the_rest_and_keeps_the_project`
  (no criterion names this case; added in review round 1) shows the window
  saving the rest, keeping the project, saying *Saved. The project stays
  here: unsaved work from an earlier session or unfinished fields still
  wait for you.*, and the agent hearing *the window has unsaved work*. The
  person decides a held draft where the window always offers it: the
  banner above the document's text, *Keep unsaved work* or *Discard it*;
  the agent's next write then meets a clean window, or the prompt again.
  Unfinished inspector fields have no test here.
- Windows, beyond the CI test run.
- Screen reader reading of the prompt.

## Review round 1

The automated review of #180 (cubic) found five points, answered on the
pull request. Fixed: the prompt now bounds the agent's name and strips
control characters as the bar does (`agent_name`, with a unit test);
criterion 2's test now asserts that the write that waited during *Save all*
answers `stale_source` (it was based on the version before the person's
save) instead of only "not a hand-over code" — a stronger form of the same
check, made after the implementation; the held-draft case of *Save all*
has a test; this report's criterion 1 row and the held-draft note; a doc
comment of `save_all_then`. Nothing drawn changed; the captures were not
retaken.

## Regression smoke list

Linux, 2026-11-01, release binaries of `d245c9c`:

| # | Result |
| --- | --- |
| 2, 10 | `--screenshot` of the demo shop in Code, Diagram and Split at 960 × 540 and 1600 × 1000: all six captures at the requested size, the tree, the editor and the board shown, all controls visible (not committed) |
| 1, 3–9 | Not run on the release binary for this ticket; the workspace suite above covers their tests |

The owner's Windows run is pending as for the earlier tickets.
