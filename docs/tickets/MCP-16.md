# MCP-16: The observing window follows the agent's edits live; following can be unpinned in the bottom bar

- Size: M · Owner review: 30 min · Risk: `visible UI`, `default`
- State: Analysis (issue #183) · decisions 1–7 pending · mock-up pending ·
  after MCP-10 (#94) · an addition of 2026-11-01, charged to the UX
  capacity line (30 min)
- Analysis model: Fable · Implementation model: —

## Goal

While an agent holds the project, the window shows live the files the agent
is editing: it follows the agent to the document it writes, creates or
publishes, so the person watches the work land without clicking after it.
An option in the bottom bar unpins the continuous following when the person
wants to stay where they are.

## What the owner will see

- New: a *Follow* toggle in the bottom bar, left of *Take back*, pressed
  while following. While it is pressed every commit the agent makes
  activates the tab of the document it changed (opening it beside the
  active tab when it has none), keeps the view the person is in (Code,
  Diagram or Split), scrolls the code to the first changed line and marks
  the changed lines in the gutter until the agent's next commit to that
  document. A document the agent creates opens at its top; a published
  component's tab comes to the front showing the new revision. Clicking
  another tab, or a file in the explorer, unpins following; clicking
  *Follow* pins it again and jumps to the agent's last change at once.
- Changed: MCP-08a's rule *the active tab, camera and selection stay across
  a reload* now applies only while following is unpinned. The camera of the
  diagram and the selected block are kept in both states; only the active
  tab and the code's scroll move while following.
- Removed or hidden: nothing.
- Migrated or converted: nothing; no setting is persisted, no file changes.
- Default behaviour that changes: following is **on** whenever the window
  starts observing (decision 1). Today the window keeps the person's place.

## Decisions for the owner

A recommended in each; a paragraph at most.

1. **Following on or off when observing starts.** **A:** on, every time the
   window becomes the observer (hand-over or a window opened while the agent
   holds the project); unpinning lasts until the person pins again or the
   observing ends. Consequence: the person who handed the project over sees
   the agent's first write land without a click. **B:** off; the person
   presses *Follow* when they want it. Consequence of B: one click per
   agent session, and the first change is easy to miss.
2. **Where following takes the window.** **A:** to the document's tab —
   opening it beside the active tab when it has none — in the view the
   person is in; a mode is never switched for them. **B:** the same, but the
   view switches to Code for a source change and to Diagram for a
   layout-only change. Consequence of B: the view flips while the person
   reads. **C:** only among the tabs already open; a change in a document
   without a tab shows in the bar's last-change text only. Consequence of
   C: a created document is never shown without a click.
3. **What the view does with the change.** **A:** the code scrolls so the
   first changed line is visible and the changed lines carry a mark in the
   gutter until the agent's next commit to that document; in Diagram the
   camera and the selection stay (the sealed layout is shown whole).
   **B:** scroll only, no mark. **C:** neither; the tab is enough.
   Consequence of C: a change below the fold is invisible until scrolled
   to.
4. **What pauses following.** **A:** the person's own navigation to another
   document — a tab click, a file in the explorer, Ctrl+Tab, closing the
   followed tab — unpins following, and the *Follow* toggle unpins or pins
   it explicitly; pinning jumps to the agent's last change. Scrolling,
   zooming, selecting a block or switching Code/Diagram/Split does not
   unpin. Consequence: the window never fights a person who has gone
   elsewhere. **B:** only the toggle unpins; the next commit brings the
   person back to the agent's file wherever they clicked. Consequence of B:
   a person reading another file is pulled away every few seconds.
5. **The control.** **A:** a compact toggle button *Follow* with a pin icon
   at the right end of the bar, left of *Take back*, pressed while
   following; hover text *Follows the agent to every file it changes; click
   to stay where you are* / *Click to follow the agent again*; accessible
   name *Follow the agent* with its on/off state. **B:** a checkbox *Follow
   the agent* in the same place. **C:** a text link *Unpin* / *Pin* inside
   the status text. Consequence of C: no visible state, and it moves with
   the text's length.
6. **Tabs that following opened when the agent finishes.** **A:** kept —
   they are the person's tabs now and a record of what the agent touched;
   they close as any tab does. **B:** closed when the agent finishes unless
   the person activated them. Consequence of B: tabs vanish on their own,
   which nothing else in the window does.
7. **Runs.** **A:** a run does not move the view; MCP-10's status line names
   the file that runs. **B:** the start of a run also brings that document's
   tab to the front. Consequence of B: an agent that edits one file and
   runs another makes the window alternate between them.

Publish, create and delete follow one rule under decisions 2 and 3: a
created document opens at its top with no marks; a published component's
tab comes to the front with the new revision and no scroll; a deleted
document's tab closes with MCP-08a's notice and following goes nowhere; a
commit of project settings alone moves nothing.

## Mock-up

Pending: review captures from the throwaway branch `mock/mcp-16-follow`
(form 1 of [planning](../process/planning.md#mock-ups)), posted on #183.
**Approval of the mock-up is required before implementation.** The capture
of each state is a `review_*` test rendering static content; MCP-10's
status is a placeholder string in the mock-up, since MCP-10 is still on its
branch. The states, with their file names under
`docs/qa/<date>/MCP-16/`:

| Capture | Size · view | Bar, from the left | Workspace |
| --- | --- | --- | --- |
| `f1-following-code-960x540` | 960 × 540 · Code | holder text in its short form *Claude Code · since 10:42*, MCP-10's status *Agent: running promotion-and-cart-price.afs (simulation)…*, *Follow* pressed, *Take back* | the followed scenario's tab active, scrolled so lines 12–14 are visible and marked as changed in the gutter |
| `f2-unpinned-code-960x540` | 960 × 540 · Code | full holder text, last-change text *10:43 · promotion-and-cart-price.afs saved*, *Follow* released, *Take back* | a different tab active than the one the last change names; no marks |
| `f3-following-diagram-1600x1000` | 1600 × 1000 · Diagram | full holder text, MCP-10's status *Agent: promotion-and-cart-price.afs passed · 18 steps · 12:58*, *Follow* pressed, *Take back* | the board as M1, camera unchanged |
| `f4-following-split-960x540` | 960 × 540 · Split | as `f1` | code with the marks, board beside it |

The captures prove: the status, *Follow* and *Take back* are whole at 960
px in every view and the holder text gives way (short form, then truncated
with the full text on hover); the pressed and released states of *Follow*
are distinguishable; the gutter marks are visible beside the diagnostics'
colouring and do not move the text. Nothing else of M1 or M4 changes.

## Acceptance criteria

Each test is in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`,
`mod mcp16`, with the server spawned as `mod mcp08b` does, unless it says
otherwise.

1. The window becomes an observer (hand-over, and a window opened while the
   agent holds the project): *Follow* is pressed. Test:
   `mcp16_criterion_1_following_is_on_when_observing_starts`.
2. The agent saves a document whose tab is not active: within two seconds
   its tab is active, opened beside the previously active tab when it had
   none; the mode is unchanged (one test per mode); in Code and Split the
   first changed line is in view and the changed lines are marked; in
   Diagram the camera and the selected block are unchanged. The marks go
   with the agent's next commit to that document. Test:
   `mcp16_criterion_2_a_saved_document_is_followed_in_{code,diagram,split}`.
3. The agent creates a document: within two seconds its tab is open and
   active at the top, no marks. Test:
   `mcp16_criterion_3_a_created_document_is_followed`.
4. The agent publishes a component: its tab comes to the front showing the
   new revision; the code's scroll is unchanged. Test:
   `mcp16_criterion_4_a_publication_is_followed`.
5. The agent deletes the followed document: its tab closes with MCP-08a's
   notice and the active tab is the one `workbench.close` chooses, not
   another document of the commit; a commit of project settings alone
   changes no tab. Test: `mcp16_criterion_5_delete_and_settings_move_nothing`.
6. A run (`run` message, started and ended) changes no tab and no scroll.
   Test: `mcp16_criterion_6_a_run_moves_nothing` (decision 7 = A).
7. Unpinned by the toggle: the next commits keep the active tab, the scroll,
   the camera and the selection as MCP-08a promises; the bar's last-change
   text still names the change. Test:
   `mcp16_criterion_7_unpinned_keeps_the_place`; MCP-08a's own tests of
   criteria 2 and 3 unpin first and keep asserting the place.
8. Auto-unpin: while following, a tab click, a file activated in the
   explorer, Ctrl+Tab and closing the followed tab each release *Follow*;
   scrolling, zooming, selecting a block and switching the mode do not.
   Pressing *Follow* again activates the document of the agent's last
   commit at once. Test: `mcp16_criterion_8_navigation_unpins_and_follow_pins`.
9. The toggle is named *Follow the agent* for assistive technology with its
   on/off state, is reachable by Tab inside the bar, and carries the hover
   text of decision 5. Test: `mcp16_criterion_9_the_toggle_is_accessible`
   (`accessibility_tests.rs` pattern).
10. At 960 × 540 in Code, Diagram and Split with MCP-10's status present,
    *Follow*, *Take back* and the status are whole and disjoint and the bar
    stays inside the window; the holder text gives way. Test:
    `mcp16_criterion_10_the_bar_fits_at_the_minimum` with
    `assert_within_window` and `assert_disjoint`; the QA captures compare
    against `f1`–`f4`.
11. Tabs opened by following stay when the agent finishes and after *Take
    back*; the followed document is editable then. Test:
    `mcp16_criterion_11_followed_tabs_stay`.
12. A second agent session in the same window starts with following on
    again after the person unpinned it in the first. Test:
    `mcp16_criterion_12_following_resets_per_observation`.
13. The QA report compares the implementation against the approved mock-up
    in all four states; the verification commands of the development guide
    pass.

## Not in scope

Clicking MCP-10's status line to open the file that runs; a list or history
of what the agent touched; following the agent's run output into *Run
results*; a keyboard shortcut for the toggle; remembering the unpinned
state across sessions or projects; following when the window is the
writer (there is nothing to follow). Following applies to every holder's
commits — `apply`, another window, an agent — as one observer state
(MCP-00 decision 12); the toggle shows whenever the bar does.

## Estimate

M: complexity is real and bounded (the observer reload path, the bar and
the code view's scroll meet in one feature; a state machine of two states);
regression potential is MCP-08a's keeping-the-place, covered by criterion
7 and MCP-08a's own tests; verification is three modes at two sizes with
MCP-10's status. 30 owner minutes: a visible change the owner will want to
use locally with a real agent. If decision 3 = A proves heavy in
implementation, the gutter marks split off as MCP-16b and the ticket ships
with scroll only; nothing else is split. Dependency: **after MCP-10** (#94,
on `feat/mcp-10-run-status`): the bar is shared and the 960 px rule of
MCP-10 is extended here; the mock-up can be rendered now with a placeholder
status, the implementation branches from `main` once MCP-10 is merged.

## Technical appendix

**How the window knows which document the agent touched.** It already
does: in observer mode every commit is followed by a reload from one stable
read, and `ProjectSession::commit_changes` (`crates/apiforge-project/src/external.rs`)
diffs the session before and after into `ExternalOutcome.followed:
Vec<CommitChange { path, document_id, kind }>` with kinds `Published`,
`Added`, `Deleted`, `Saved`, `Layout`, `Settings`. `FolderApp::follow_external`
(`crates/apiforge-desktop/src/folder/mod.rs`) consumes it today for the
bar's last-change text (`describe_commit`) and the closing of deleted tabs.
The `saved { generation, paths }` message of the channel
(`crates/apiforge-channel/src/lib.rs`, `VERSION` 1) stays the trigger for
the immediate rescan and nothing more; its `paths` are not used, since the
reload's diff is authoritative and also covers `apply` and another window.
**No channel message changes; the protocol version stays 1; no file format
or persisted setting is added.**

**Which change to follow.** One commit may carry several changes
(`create_document` adds the source, then seals the layout; a publication
may save the component too). Priority: the document of the first change in
the order `Added`, `Saved`, `Published`, `Layout`; `Deleted` and `Settings`
never move the view. The sort in `commit_changes` is by kind, so the window
sorts again by this priority rather than taking `followed[0]`.

**The changed lines.** `commit_changes` has both sessions; for a `Saved`
change it computes the first and last changed line by common prefix and
suffix of the old and new text (O(n), no dependency) into a new
`CommitChange::lines: Option<Range<usize>>`. The window scrolls with the
code editor's `reveal(at)` (`crates/apiforge-desktop/src/code.rs`, used by
*Open in code*) to the first changed line without selecting, and paints
the marks in the gutter the editor already draws for line numbers; the
marks are kept per document in the folder app and cleared by the next
followed commit to that document. A `Layout` change has no lines.

**State.** `Follow { pinned: bool, last: Option<String> }` beside
`handover` in `FolderApp`; set to the default when `observing()` turns
true, cleared when it turns false. Following navigates through
`workbench.open` directly; the person's navigation goes through
`activate_tab` (explorer, tab strip, diagram's *Open in code*), `cycle`
and `close_tab`, which unpin while observing. `activate_tab` is also called
by the window's own restore on open; the implementing agent gates the
unpin on the gesture, not the function.

**The bar.** `holder_bar` in `crates/apiforge-desktop/src/folder/holder.rs`
lays out right-to-left: *Take back*, then *Follow* (the window's compact
action button, `widgets::action(Icon::Pin, "Follow")`, drawn selected while
pinned; `Icon::Pin` is new in `icons.rs`), then the last-change or MCP-10
status text, then the holder text. The 960 px rule: the holder text takes
its short form first (MCP-10), then truncates with the full text on hover;
the status and the buttons are never cut.

**Tests.** `mod mcp16` in `acceptance_tests.rs` with `mod mcp08b`'s spawned
server; the `mcp08a` tests that assert the active tab after a commit to
another document press the toggle first. Design review views for the four
states live in `testing::review` and are the mock-up's source.
