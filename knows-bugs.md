# Known bugs — exploratory desktop QA

Test date: 2026-10-24. The original exploratory report below describes revision
`854b44f`. **Fixes for BUG-001–017 are now implemented in the working tree**, with
regression tests and selected native UI retests. See the
[fix verification report](docs/qa/2026-10-24/fixes/verification.md) for coverage and
limits. A subsequent review fixed **BUG-018–025**, recorded in the
[additional verification report](docs/qa/2026-10-24/fixes-next/verification.md).
A third pass, prompted by a report that Backspace does not work in the code
editor, found and fixed **BUG-026–031** in the editor's own key handling and
drawing; see the [editor verification report](docs/qa/2026-10-24/fixes-editor/verification.md).
A fourth pass ran the whole application end to end — the demo API, the command
line against it and the native window — and fixed **BUG-032–034**, in the
[end-to-end verification report](docs/qa/2026-10-24/fixes-e2e/verification.md).
Original reproductions and screenshots are retained below.

Stage II P1–P4 native QA on `main` at `8fafaae` found **BUG-035 (open, P2)**:
new folder-project diagram cards overlap. The
[P1–P4 QA report](docs/qa/2026-10-24/p1-p4-native-interactive.md) records the
interactive checks, supporting automated tests and their limits.

Tested revision: `854b44f` (`feat/live-sync` merged). Native release executable:
`target/release/apiforge-desktop`, SHA-256
`5b7868b6bb25ed265bd28081f8aa7cf73aa1a21fcdf4387cba39b138e67f14ce`.

Environment: Ubuntu 26.10, real egui application window on an isolated Xvfb X11
session, 1600 × 1000 at 96 DPI unless stated otherwise. Mouse and keyboard input
through xdotool; clipboard through xclip; screenshots through Xlib. Tools were
installed locally under `/tmp/apiforge-qa/tools`. Application data and preferences
were isolated under `/tmp/apiforge-qa/isolated-{data,config}`; the user's workspace
was not used. The host desktop is GNOME/Wayland, so KWin tools do not apply.

Priorities: **P1** = lost edits or incorrect execution; **P2** = broken workflow;
**P3** = polish/accessibility. Findings require a reproducible UI observation;
source references are investigation hints, not substitutes for reproduction.

## Triage summary

**17 findings: 8 P1, 7 P2, 2 P3.** Fix loss of edits and incorrect execution first;
review visual polish alongside the locking and panel-layout work.

| ID | Priority | Finding |
| --- | --- | --- |
| BUG-001 | P1 | First code edit is silently lost |
| BUG-002 | P1 | Invalid-code lock permits edits that are later deleted |
| BUG-003 | P1 | Enter in the inspector accepts a stale code completion |
| BUG-008 | P1 | Editing another project removes persisted drafts |
| BUG-009 | P1 | Immediate Run executes earlier code |
| BUG-013 | P1 | Unfinished body/URL inspector drafts disappear on restart |
| BUG-016 | P1 | Stale draft overwrites newer document before conflict choice |
| BUG-017 | P1 | Whole-code Undo restores only cleanup when its tab is active |
| BUG-005 | P2 | Panels and controls overlap at the supported minimum size |
| BUG-006 | P2 | Component code adds overlapping blocks |
| BUG-007 | P2 | Saving a component silently ignores invalid code |
| BUG-011 | P2 | Correcting a URL in code leaves a stale invalid inspector field |
| BUG-012 | P2 | Body completion inserts syntax rejected by its own editor |
| BUG-014 | P2 | Code selection fails across main and cleanup phases |
| BUG-015 | P2 | Open in code leaves its target outside the visible code |
| BUG-004 | P3 | Heavy grey invalid-code state makes the board hard to read |
| BUG-010 | P3 | Completion popup detaches from its caret |

Found in the third pass, on the same areas:

| ID | Priority | Finding |
| --- | --- | --- |
| BUG-026 | P1 | Backspace cannot delete indentation; it is written straight back |
| BUG-027 | P1 | Tab deletes a selected block instead of indenting it |
| BUG-028 | P2 | Tab and Shift+Tab do not use the language's indentation |
| BUG-029 | P2 | The line-number gutter makes a long buffer unusable |
| BUG-030 | P3 | Auto-indent ignores brackets in a request body |
| BUG-031 | P3 | The code panel reprints the whole document every frame |

Found in the fourth pass, running the application end to end:

| ID | Priority | Finding |
| --- | --- | --- |
| BUG-032 | P2 | An ordering wire runs under the block it leaves, so nothing shows where it starts |
| BUG-033 | P2 | A failed component prints no inner steps, though its message says to read them |
| BUG-034 | P3 | A failed comparison at the command line never says what it expected |

Found in Stage II P1–P4 folder-project QA on `8fafaae`:

| ID | Priority | Status | Finding |
| --- | --- | --- | --- |
| BUG-035 | P2 | Open | Newly analysed folder-project cards overlap in Diagram and Split |

## BUG-001 — P1 — The first code edit in a fresh buffer is silently lost

**Reproduced twice**, in two newly created scenarios.

1. Open Code (Ctrl+E).
2. Create a scenario with the sidebar `+`, name it `QA first edit`, and wait for it
   to appear. Its initial code contains `on_error continue` and `begin = start`.
3. Click inside the code editor, select all, and paste:

   ```text
   scenario "QA first edit" { answer = 42 }
   ```

4. Wait one second.

**Actual:** the original text returns. The board still contains `begin`, with no
`answer` block. Copying the editor text confirms the original code, not the pasted
code. There is no error, warning or recovery prompt. Retrying the edit after this
first attempt works.

**Expected:** the first edit must be retained and applied after the 250 ms typing
pause, exactly like later edits.

**Impact:** the main new feature can discard a user's entire pasted scenario on
its first use. This also affects confidence in typing and imports via clipboard.

**Evidence:** [editor after the lost paste](docs/qa/2026-10-24/BUG-001-first-edit-lost.png).

**Investigation:** `crates/apiforge-desktop/src/buffers.rs`, `Buffers::typed`,
`Buffers::text` and `Buffers::apply`. A typed buffer stays `Clean` until applying;
`text()` replaces a `Clean` buffer with the printed document on the following
frame, before the debounce expires.

## BUG-002 — P1 — Invalid code does not actually prevent document edits; fixing code deletes them

1. Use an applied scenario, for example:

   ```text
   scenario "QA" {
     value = 42
   }
   ```

   If necessary retry the first paste because of BUG-001.
2. Append `broken =` after the closing brace and wait for the error banner.
3. Observe `The code has errors, so the board waits for it.` and the disabled
   canvas.
4. Click `Add step` above the canvas and choose `Value`.
5. Fix the code by removing `broken =` and wait for synchronisation.

**Actual:** step 4 succeeds: the graph gains a block, the inspector opens, and
changes are saved even though the canvas is disabled. The invalid text does not
contain that new block. Step 5 replaces the graph from the old text and silently
removes the block added through the UI. In the tested four-block scenario the
counter changed **4 → 5 → 4**.

**Expected:** all document mutations in this context must be blocked while code
is invalid, including toolbar, inspector, library, undo/redo and keyboard actions,
as required by ADR 0007. Fixing code must not silently erase permitted UI edits.

**Impact:** the visual lock gives a false guarantee; mixing the still-enabled
controls with code edits loses work.

**Evidence:** [block added while code is invalid](docs/qa/2026-10-24/BUG-002-invalid-added.png),
[block gone after fixing code](docs/qa/2026-10-24/BUG-002-added-block-lost.png).

**Investigation:** `crates/apiforge-desktop/src/app.rs`: `board_waits` disables only
the `CentralPanel`; the editor toolbar and other mutation paths are outside that
scope. Guard document-changing commands centrally, rather than only dimming the
canvas.

## BUG-003 — P1 — Completion stays open after blur and Enter in another field changes code

1. Apply this code (retry the first paste if BUG-001 occurs):

   ```text
   scenario "QA code" {
     response = GET "/" { sample 200 { alpha: 1, beta: 2 } }
     result = response.body
   }
   ```

2. Put the caret after `response.body` on the last assignment and type `.`.
   Completion offers `alpha` and `beta`.
3. Click outside the code editor. The popup remains visible.
4. In the `result` block's inspector, change **Block name** to `renamed`.
5. With the inspector field still focused, press Enter.

**Actual:** Enter accepts `alpha` in the unfocused code editor. The code changes to
`result = response.body.alpha`, becomes valid and applies. The block's meaning
changes from the entire response object to a number, and the inspector rename is
also lost: `renamed` becomes `result` again. Focus remains in the inspector.

**Expected:** leaving the code editor dismisses its completion popup. Only the
focused editor may claim its completion keys. Enter in an inspector field must
not mutate code in another panel.

**Evidence:** [inspector focused, stale popup still visible](docs/qa/2026-10-24/BUG-003-completion-before.png),
[Enter changes code and removes the rename](docs/qa/2026-10-24/BUG-003-completion-after.png).

**Investigation:** `crates/apiforge-desktop/src/code.rs`,
`Completions::take_keys` / `update` / `draw`. Key consumption and popup lifetime
are not gated on the associated text edit's focus.

**Further focus-related reproduction (continued QA):** Ctrl+Space in the
inspector body does not open its suggestions even when the caret is after
`param.`. Typing the dot does open the expected automatic popup. An earlier
rendered editor consumes the manual-completion shortcut without owning focus;
closing Code alone is insufficient because the URL editor is also drawn first.
[Manual completion missing in the focused body](docs/qa/2026-10-24/BUG-003-manual-completion-missing.png),
[automatic completion for the same prefix works](docs/qa/2026-10-24/BUG-012-body-completion.png).

## BUG-004 — P3 — Invalid-code state covers the board with a heavy grey disabled surface

This is a **visual/UX finding**, independently of the functional lock bypass in
BUG-002. It was also explicitly called out by the user during this QA session.

1. Open Code beside a populated board.
2. Enter incomplete code and wait for the error banner.

**Actual:** the whole board becomes a flat dark grey area, with pale cards, very
low-contrast labels, wires and controls. It resembles an inactive modal backdrop
or an unresponsive application. The error explanation is confined to the code
panel; the board itself does not explain its state. Toolbar/inspector controls
remain bright and apparently available, creating inconsistent visual signals.

**Expected/design acceptance:** keep the last valid graph readable as a useful
reference while fixing code. Use a deliberate read-only treatment with a concise
status message on the board, consistent control states and a visible recovery
action. Avoid turning the main work area into a large low-contrast grey slab.
The final treatment should be reviewed with short and long errors, an open
inspector, run results, and the minimum supported window.

**Evidence:** [invalid board with inspector](docs/qa/2026-10-24/BUG-003-completion-before.png),
[invalid board with results](docs/qa/2026-10-24/BUG-002-invalid-added.png).

**Additional UX failure (continued QA):** closing Code while its text is
invalid hides both the only error explanation and the Discard code changes
button. The user is left with a disabled grey board, no error badge on Code and
no visible reason for the block or direct recovery action.
[Entire disabled work area with the explanation hidden](docs/qa/2026-10-24/BUG-004-hidden-error-grey-board.png).

## BUG-005 — P2 — Code and inspector leave a nearly zero-width canvas at the supported minimum

1. Open Code.
2. Select a block and show its inspector.
3. Resize the native window from 1600 × 1000 to **960 × 540**, the documented
   minimum supported logical size (96 DPI in this test).

**Actual:** sidebar + code panel + inspector consume almost all available width.
The board becomes an approximately **29 px** strip. `Add step` and other board
controls overflow onto the inspector, covering its title; zoom controls overlap
its content. The board cannot be meaningfully inspected or edited.

**Expected:** responsive layout must reserve useful board space and avoid control
overlap. At narrow widths, collapse a panel, use a drawer/tab mode, or offer an
explicit switch between code, board and inspector. Verify resizing down from a
wide window with panels already open, not just initial startup at 960 × 540.

**Evidence:** [minimum supported window](docs/qa/2026-10-24/BUG-005-minimum-window.png).

**Investigation:** `crates/apiforge-desktop/src/app.rs`, panel allocation in the
main UI. The compact code panel's `default_size` does not shrink an existing
wide panel, and the sum of panel minima leaves insufficient canvas space.

## BUG-006 — P2 — Blocks added through component code are stacked on top of one another

1. In the demo shop scenario, select `Product`, click **Open component**, then
   open Code.
2. Before the component's final `}`, add:

   ```text
   extra_one = 1
   extra_two = 2
   ```

3. Wait for the code to apply (retry the first edit if BUG-001 occurs).
4. Close Code to inspect the whole board.

**Actual:** the count correctly increases from 2 to 4, but `extra_one` and
`extra_two` occupy the same position. Only `extra_two` is visible; it also
obscures the existing request card. Selecting or dragging the covered block is
unnecessarily difficult.

**Expected:** newly introduced component blocks receive separate, non-overlapping
positions while existing blocks keep their layout, just as for scenario code.

**Evidence:** [four blocks, with new cards stacked](docs/qa/2026-10-24/BUG-006-component-overlap.png).

**Investigation:** `App::apply_code` calls `place_new_blocks` only for scenarios;
the component branch applies the reconciled graph without placing fresh nodes.

## BUG-007 — P2 — Save revision silently ignores invalid component code

1. Open a component, make a valid code edit and wait for it to apply.
2. Append `broken =` after its final brace and wait for the error banner.
3. Click **Save revision**, then **Save component** in the metadata dialog.

**Actual:** both actions remain enabled. A new revision is saved from the last
valid graph, the app returns to the scenario, and the only feedback says
`Saved “Product”, revision 2. Existing instances keep their revisions.` There is
no warning that the visible code was invalid or that its latest edits were not
included. In the test the persisted revision contained the four last-valid nodes
and omitted the erroneous text.

**Expected:** block saving until code is valid, or explicitly ask whether to save
the last valid version. Keep the invalid draft reachable and make the difference
between saved code and visible code clear. Scenario Run already has such a
confirmation, so component saving should provide equivalent clarity.

**Evidence:** [save dialog over invalid code](docs/qa/2026-10-24/BUG-007-invalid-save-dialog.png),
[unqualified success notice](docs/qa/2026-10-24/BUG-007-invalid-save-success.png).

**Investigation:** the component `Save revision` path in `App::scenario_header`
constructs its dialog from `draft.graph` without checking the code buffer.

## BUG-008 — P1 — Editing a second project removes the first project's persisted drafts

1. In Demo shop, create an invalid scenario draft with a distinctive marker:

   ```text
   // QA SHOP DRAFT
   broken_shop =
   ```

   Append this after the scenario's final brace; verify that the error banner and
   marker remain visible. Retry the first edit if BUG-001 occurs.
2. Wait for persistence. `drafts.json` contains this `shop` entry.
3. Switch the workspace dropdown to **Ticket system**.
4. In its scenario, append a different invalid marker (`broken_ticket =`) and
   wait for persistence.
5. Close the application normally and reopen it with the same data directory.
6. Return to Demo shop and the scenario edited in step 1.

**Actual:** after step 4, `drafts.json` contains only the `tickets` project entry;
all `shop` entries are gone. After restarting, the ticket draft is restored, but
the shop scenario shows only its last valid printed code. `QA SHOP DRAFT` and
`broken_shop =` have been permanently lost without a warning. The test also lost
the shop component draft and a differently formatted valid scenario draft.

**Expected:** drafts from every project must coexist in the persisted file and
survive project switching plus a normal restart, as promised by ADR 0007.

**Evidence:** [shop draft before switching](docs/qa/2026-10-24/BUG-008-shop-before.png),
[shop after restart, draft gone](docs/qa/2026-10-24/BUG-008-shop-after-restart.png).
The `tickets` draft was successfully restored in the same restart, ruling out a
general failure to load drafts.

**Investigation:** `App::save_drafts` filters entries with
`if project_id != project { return None; }`, then overwrites the shared draft
file with that filtered set. Draft lookup/printing must cover all workspace
projects, not only `self.editor.project()`.

**Second reproduction without editing the other project (continued QA):**
create an invalid shop draft, switch to Ticket system without changing its code,
then close and restart. Before close the active project was `tickets` and the
shop marker was still in `drafts.json`. After reopening and returning to shop,
the marker was absent from the code panel even though it was **still present in
the draft file**. Thus preserving the file alone will not fix restoration:
`restore_drafts` also filters against only the startup project's ID and never
loads other projects when they are later selected.
[Shop draft ignored after starting on another project](docs/qa/2026-10-24/BUG-008-inactive-project-not-restored.png).

## BUG-009 — P1 — Run immediately after a code edit executes the earlier version without asking

1. Apply this valid scenario and wait:

   ```text
   scenario "QA code" { value = 1 ok = assert value == 1 }
   ```

2. Replace it with:

   ```text
   scenario "QA code" { value = 2 ok = assert value == 1 }
   ```

3. Click **Run test immediately**, before the 250 ms code-apply delay expires.
   The reproduced input sequence used approximately 150 ms between paste and
   click.

**Actual:** the run **passes**, and its output shows `value: 1`. Meanwhile code
and board show `value = 2`; the report then says `Graph changed · results from an
earlier version`. There was no prompt before executing the earlier version.

**Expected:** Run must first settle and validate pending code, wait for it to
apply, or explicitly ask to run the previous version. With the code in step 2,
the resulting current-version assertion must fail. A small timing difference
must not silently select a different scenario to execute.

**Impact:** the new editor can produce a false passing test; in live mode the
same execution-selection issue could send requests from the earlier code (live
side effects were not used to test this finding).

**Evidence:** [board value 2, passed run output 1](docs/qa/2026-10-24/BUG-009-stale-run.png).

**Investigation:** `App::start_run` checks whether a buffer is `Invalid`, but
ignores edits still pending the debounce; the toolbar runs before the later
`code_panel` apply step.

## BUG-010 — P3 — Completion popup becomes detached from its caret when the error banner appears

1. Apply the two-block response scenario from BUG-003.
2. Type `.` after `response.body` and leave completion open.
3. Wait for validation to insert the error banner above the code editor.

**Actual:** the editor and its caret move down to make room for the banner, but
the popup stays at its old screen position. In the captured 1600 × 1000 window,
the assignment/caret is around y=293 while the popup starts around y=202. It
partly overlaps the error banner and looks unrelated to the expression being
completed.

**Expected/design acceptance:** anchor the popup to the current caret rectangle
on every layout update, including appearance/disappearance of diagnostics,
scrolling and resizing. Avoid covering the relevant code or recovery controls.

**Evidence:** [popup above the line it completes](docs/qa/2026-10-24/BUG-010-popup-detached.png).

**Investigation:** `Completions::update` records `Popup::at` only when opening or
refreshing suggestions; `draw` later reuses that fixed position.

## BUG-011 — P2 — A corrected code URL leaves an obsolete invalid URL in the inspector

1. Apply a scenario with `request = POST "/orders"` and select that request.
2. In its inspector URL field enter `/unfinished/{{missing}}`. The field correctly
   reports that no block is named `missing` and keeps the saved URL unchanged.
3. Open Code and change the same request's URL to `/updated`. Wait for it to apply;
   retry the first code edit if BUG-001 occurs.
4. Return focus to the inspector URL field.

**Actual:** the code and request card now show `/updated`, while the inspector
still shows `/unfinished/{{missing}}` and its old error. There is no indication
that the underlying URL changed, and no action for discarding/rebasing this stale
field draft. Merely leaving and returning to the field does not resolve it.

**Expected:** reconcile the field with changes from code/undo, or explicitly
present a draft conflict with keep/discard actions. The three views of the request
must not silently diverge without explaining which URL is current.

**Evidence:** [corrected code/card versus obsolete inspector](docs/qa/2026-10-24/BUG-011-stale-url.png).

**Investigation:** `inspector/fields.rs`, `TemplateDraft` / `template_field` caches
text/error without retaining and comparing its source binding. Unlike `BodyDraft`,
it cannot detect an external update to that field while an invalid draft exists.

## BUG-012 — P2 — Body completion inserts expressions that the JSON editor rejects

1. Create a scenario with `param quantity = 3` and a POST request with JSON body.
2. In the inspector body field enter `{ "quantity": param. }`, placing the caret
   immediately after the dot. Type the dot rather than only invoking Ctrl+Space.
3. Completion offers `quantity`, labelled `a scenario parameter`.
4. Press Enter to accept it.

**Actual:** completion produces `{ "quantity": param.quantity }`, immediately
rejected as `Invalid JSON body at line 1, column 15 — the last valid body is kept.`
The editor itself recommends an insertion it cannot apply. The accepted syntax
in this field requires a placeholder such as `{{param.quantity}}`; bare scenario
expressions work in the scenario code panel, not this JSON projection.

**Expected:** use a single consistent body language, or make completion insert
valid placeholders in JSON mode. Do not offer raw expressions at a JSON value
position unless the body parser can accept them.

**Evidence:** [offered parameter](docs/qa/2026-10-24/BUG-012-body-completion.png),
[accepted suggestion still invalid](docs/qa/2026-10-24/BUG-012-invalid-insertion.png).

**Investigation:** the widget uses `complete::Mode::ObjectBody`, whose suggestions
are expression-oriented, but parses the result through
`apiforge_core::body_text::parse_json_body`, which expects JSON plus placeholders.

## BUG-013 — P1 — Unfinished inspector body and URL edits disappear on normal restart

1. Edit a request's body in its inspector. Enter recognisable unfinished JSON:

   ```text
   {
     "note": "QA UNSAVED BODY",
     "quantity":
   ```

2. Verify that the text and its validation error remain visible. Optionally also
   leave an invalid URL such as `/unfinished/{{missing}}` in the URL field.
3. Wait at least one second, then close the application normally.
4. Reopen the same workspace, return to the scenario, select the request and
   scroll to its body.

**Actual:** no close warning is shown. The unfinished body is gone and the
inspector shows only its last valid saved body. The invalid URL draft is also
gone. In the test, `QA UNSAVED BODY` disappeared and body returned to
`{ "quantity": "{{param.quantity}}" }`. The workspace status before closing said
`Saved locally`; no indication distinguished these temporary field drafts.

**Expected:** preserve unfinished text across normal close/restart, or warn and
provide a concrete way to keep/recover it before discarding. The advertised draft
protection must have a consistent experience across the new code-editing surfaces.
This reproduction uses a single project, independently of BUG-008.

**Evidence:** [unfinished body before closing](docs/qa/2026-10-24/BUG-013-body-before-restart.png),
[last valid body after restart](docs/qa/2026-10-24/BUG-013-body-after-restart.png).

**Investigation:** `BodyDraft` and `TemplateDraft` exist only in egui temporary
widget data. Workspace draft persistence iterates `buffers::Buffers` and does not
include these inspector drafts.

## BUG-014 — P2 — Code selection does not follow blocks between main and cleanup phases

1. Apply this scenario:

   ```text
   scenario "QA code" {
     main = 41
     cleanup {
       teardown = main + 1
     }
   }
   ```

2. Keep the **Scenario** board tab active. Click the `main` assignment in code,
   then click the `teardown` assignment inside `cleanup`.
3. Manually switch the board to **After test**, then click the `main` assignment
   in code.

**Actual:** step 2 leaves the main block selected and its inspector open; the
cleanup block is not shown. Step 3 leaves the board on the cleanup block while
the caret is in main code. The code panel represents the entire scenario, but
selection follows only blocks in the currently visible phase.

**Expected:** a statement selected in code should reveal/select its block and
switch the board to the corresponding phase. The selected block's inspector must
not remain on an unrelated statement because its tab is currently active.

**Evidence:** [cleanup line with main block still shown](docs/qa/2026-10-24/BUG-014-cleanup-caret-main-board.png),
[main line with cleanup board still shown](docs/qa/2026-10-24/BUG-014-main-caret-cleanup-board.png).

**Investigation:** `App::sync_selection` looks up a parsed handle only in
`self.editor.graph().nodes`, which contains the active phase, rather than resolving
both graphs and changing phase as needed.

## BUG-015 — P2 — Open in code leaves the requested statement off screen in long scenarios

1. Apply a scenario containing approximately 79 short Value assignments followed
   by a POST request with a body. This puts the request after line 80.
2. Scroll the code panel to the top.
3. Use Find step to select the request and scroll its inspector down to Body.
4. Click **Open in code**. Repeat with Code closed before clicking the action.

**Actual:** the code panel is open but still shows the beginning of the file,
roughly lines 1–49. The request being opened is below the viewport, with no visible
highlight or scroll to it. This happens both with an already-open code panel and
when the action opens a closed panel. Find step also reveals the board block
without bringing its code into view.

**Expected:** Open in code should scroll the target statement into view, show an
obvious caret/highlight at the request, and make editing immediately possible.
Simply changing an invisible caret is insufficient.

**Evidence:** [Open in code invoked, request still below visible lines](docs/qa/2026-10-24/BUG-015-target-offscreen.png).

**Investigation:** `inspector_action` / `sync_selection` call `CodeEditor::reveal`;
that sets `pending_caret`, but does not scroll the containing code-panel
`ScrollArea` to the caret's current rectangle.

## BUG-016 — P1 — A stale draft overwrites the newer document before the conflict choice

1. In a writable workspace apply this valid, deliberately unformatted code:

   ```text
   scenario "QA code" { value = 1 }
   ```

2. Wait for saving and close the app normally. Verify that the workspace contains
   value 1 and `drafts.json` retains the one-line text.
3. While the app is closed, change that saved node's literal value to **2** in
   `workspace.json`, preserving its IDs and leaving `drafts.json` untouched. This
   simulates another editor updating the document. In this test only the isolated
   QA copy under `/tmp/apiforge-qa/isolated-data` was changed.
4. Reopen the app and select the scenario. Do **not** choose either conflict
   action.
5. After inspecting the result, click **Discard it**.

**Actual:** the banner correctly says `This code was written before the document
changed.` and offers `Keep editing it` / `Discard it`. However, before either
choice, the board already shows **1**, and autosave has overwritten the external
value 2 in `workspace.json` with **1**. Choosing Discard it then dismisses the
banner and prints value 1; it cannot retain/restore the newer version.

**Expected:** a stale draft must remain unapplied until the user explicitly keeps
it. Discard must retain the newer saved document (value 2). Merely viewing a
conflict must never mutate or autosave over either side.

**Impact:** external edits are overwritten despite a prompt that appears to
protect them. This is independent of BUG-008's cross-project persistence issue.

**Evidence:** [conflict choice still pending, old value already applied](docs/qa/2026-10-24/BUG-016-applied-before-choice.png),
[Discard leaves the overwritten value](docs/qa/2026-10-24/BUG-016-discard-too-late.png).
Readback of the QA workspace confirmed persisted value **2 before startup → 1
before any choice → 1 after Discard**.

**Investigation:** restoring a draft sets `pending: true` and its `stale` flag, but
`Buffers::apply` does not check `stale` before reconciling and returning a document
edit. The conflict banner does not gate this normal apply path.

## BUG-017 — P1 — Undo restores only cleanup after a whole-code edit made on the cleanup tab

1. Apply this baseline scenario:

   ```text
   scenario "QA code" {
     main = 1
     cleanup {
       teardown = 2
     }
   }
   ```

2. Switch the board to **After test** while leaving Code open.
3. In code change both literals to `main = 10` and `teardown = 20`. Wait for the
   whole scenario edit to apply. Retry an edit if BUG-001 occurs.
4. Click an empty part of the canvas so keyboard focus leaves the text editor,
   then press Ctrl+Z (document Undo).

**Actual:** the cleanup value returns to **2**, but main remains **10**. Code is
reprinted as `main = 10` plus `teardown = 2`, a mixed document that was neither
the complete before nor after version. The main value 1 is not restored by
undoing the edit that changed it.

**Expected:** one whole-scenario code edit must undo both graphs and all scenario
settings atomically, regardless of which board phase is selected. The result must
be `main = 1`, `teardown = 2`.

**Impact:** Undo appears successful while leaving part of the edited scenario
changed. A later run can therefore execute behavior the user believes was undone.

**Evidence:** [After test selected, partial undo leaves main at 10](docs/qa/2026-10-24/BUG-017-partial-undo.png).

**Investigation:** `Editor::apply_scenario` uses `snapshot_with(true)`, but its
snapshot always stores the active `graph()` and includes the other graph only
when `phase == Main`. With cleanup active, the old main graph is never captured.
Whole-scenario history needs both graphs independently of the current phase.

## BUG-026 — P1 — Backspace cannot delete indentation; the editor writes it straight back

**Reported by the user; reproduced in the editor harness.**

1. Open Code on any scenario and put the caret at the end of a line that opens a
   block, for example `scenario "S" {`.
2. Press Enter. The editor indents the new line by two spaces, as it should.
3. Press Backspace twice to take that indentation away again.

**Actual:** the first Backspace removes one space. The second removes the last
one and the editor immediately puts both back, so the text returns to
`scenario "S" {\n  ` with the caret after the indentation. The line can never be
brought back to column zero, and the line break before it can never be deleted,
so two lines cannot be joined. The same happens at the start of any indented
line and with the Delete key.

**Expected:** Backspace deletes what is in front of the caret. Indentation is
added for a line the user has just broken, not for one a deletion happened to
end at.

**Impact:** every keystroke that would un-indent a line or join two lines is
silently undone. Restructuring code in the panel is impossible without deleting
and retyping whole lines.

**Investigation:** `crates/apiforge-desktop/src/code.rs`, `CodeEditor::after_edit`
ran `auto_indent` for *any* change that left the caret just after a newline. It
cannot tell an inserted line break from a deletion that uncovered one.

## BUG-027 — P1 — Tab deletes a selected block instead of indenting it

1. Open Code on a scenario with at least two indented statements.
2. Select those statements.
3. Press Tab.

**Actual:** the selected statements are **deleted** and replaced by a single tab
character. The work is gone; only text Undo brings it back.

**Expected:** Tab over a selection indents every line the selection touches and
keeps the selection, so Tab and Shift+Tab move a block in and out.

**Impact:** the most ordinary code-editor gesture destroys the selected code.

**Investigation:** the editor never handled Tab, so egui's own `TextEdit`
behaviour applied: `delete_selected` followed by inserting `"\t"`.

## BUG-028 — P2 — Tab and Shift+Tab do not use the indentation the language is written in

1. Open Code and press Tab on an empty line.
2. On a line indented by the usual two spaces, press Shift+Tab.
3. On a line indented by four spaces, press Shift+Tab.

**Actual:** step 1 inserts a tab character, which the printer never writes and
which the editor's own auto-indent does not count — the next line it indents
loses the indentation. Step 2 does nothing, because egui removes four spaces at a
time. Step 3 removes all four spaces, two of the editor's levels at once.

**Expected:** Tab moves to the next two-space stop and Shift+Tab takes exactly
one level off, matching `print_scenario` and the JSON body printer.

**Impact:** code indented with Tab is reformatted the moment it applies, and
Shift+Tab is unusable on code the application itself produced.

**Investigation:** `format.rs` writes `"  ".repeat(depth)`, while egui's
`TAB_SIZE` is 4 and its Tab inserts `"\t"`.

## BUG-029 — P2 — The line-number gutter makes a long buffer unusable

1. Open Code and paste a long document — the panel accepts up to the 1 MB the
   language allows, and keeps the text while it does not parse.
2. Scroll towards the end.

**Actual:** the window stops responding to typing and scrolling. Measured in the
editor harness on a 360 KB, 30 000-line buffer scrolled to the end: **921 ms per
frame**, in a release build. The cost grows with the square of the document, so
a megabyte takes seconds per frame.

**Expected:** the editor stays responsive at the 1 MB its own documentation sets
as the target.

**Impact:** pasting a large file into the code panel appears to hang the
application.

**Investigation:** `CodeEditor::paint_gutter` asked the galley for the position
of every line from the first one. Each answer scanned the text up to that line to
convert a byte offset into a character index, and then scanned every row before
it. Both are linear, once per line, per frame.

## BUG-030 — P3 — Auto-indent ignores brackets, so a body is indented unlike the way the editor prints it

1. Open a request body in the inspector as JSON.
2. Type `"items": [` and press Enter.

**Actual:** the new line keeps the indentation of the previous one. After `{` in
the same place it gains a level, as it should.

**Expected:** `[` opens a block of a request body exactly as `{` does, and
`print_json_body` indents both with two spaces.

**Impact:** a hand-written body does not match the body the editor prints beside
it, so opening a saved body reindents what was typed.

**Investigation:** `auto_indent` tested only `ends_with('{')`.

## BUG-031 — P3 — The code panel reprints and revalidates the whole document every frame

1. Open Code on a large scenario.

**Actual:** every frame prints the document again, which clones and validates the
whole project first. Measured: 0.2 ms at ten blocks, 2.0 ms at a hundred and
**4.5 ms at the two hundred a graph may hold**, in a release build — about a
quarter of a 60 Hz frame spent producing text that has not changed.

**Expected:** the document is printed again when it changes.

**Impact:** the panel is heaviest exactly where scenarios are largest.

**Investigation:** `App::code_panel` called `buffers::print` unconditionally.
`Editor::revision` already counts document changes; moving between documents
needed a counter of its own, because a component draft left unsaved and opened
again is the saved definition once more without any revision changing.

## BUG-032 — P2 — An ordering wire runs under the block it leaves

**Reported by the user; reproduced in the router and in the native window.**

1. Open the `Przepływ: ceny i koszyk` demo project, whose steps are joined by
   ordering wires.
2. Look at the wire between `begin` and `Pobierz ceny produktów`, two cards side
   by side.

**Actual:** the dashed wire appears above `begin`, with nothing joining it to the
`Next` port at the bottom of the card. The route is
`(180,176) → (180,84) → (510,84) → (510,100)`: it leaves the port at the bottom
edge and goes straight up **through the whole card**, which is painted over it.
Only the sixteen pixels above the card are visible.

**Expected:** a wire leaves its port perpendicular to the card side and stays in
the open, as [ADR 0006](docs/decisions/0006-layout-and-wire-routing.md) says. It
may hug a card; it may not pass behind one.

**Impact:** on a board read for its execution order, an ordering wire cannot be
traced back to the step it starts at.

**Investigation:** `crates/apiforge-layout/src/route.rs`. The two cards a wire
joins were excluded from the obstacles altogether, so the search was free to
cross them. Excluding them is only needed so a port on a card's edge has
somewhere to start, which the stub already provides.

## BUG-033 — P2 — A failed component prints no inner steps at the command line

1. Point a project's environment at a port nothing listens on.
2. `apiforge run --workspace <doc.json> --scenario <name> --live`

**Actual:** the component's line reads `error Produkt — Check the results of the
component's inner steps.` and the inner steps are never printed. The reason the
run failed — `Connection refused` on the request inside the component — appears
only with `--json`.

**Expected:** the report names the step that explains the failure.

**Impact:** a failing run in a terminal or in CI tells the reader to look at
something it does not show.

**Investigation:** `print_report` in `crates/apiforge-cli/src/run.rs` walked
`report.steps` and never `step.children`.

## BUG-034 — P3 — A failed comparison at the command line never says what it expected

1. Run a scenario whose assertion does not hold.

**Actual:** `failed  Priorytet wysoki — The comparison failed.` The report
carries `expected` and `actual`, and the window shows both side by side; the
command line prints neither.

**Expected:** the line that failed says what it wanted and what it got.

**Impact:** the one output a test runner exists to produce has to be re-run with
`--json` to be read.

**Investigation:** the same printer; `StepResult::expected` and `::actual` were
not used.

## BUG-035 — P2 — Newly analysed folder-project cards overlap

**Status: open.** Reproduced on `main` at `8fafaae`, in the real native folder
window on isolated KWin/Xwayland, at 100% zoom in Diagram and Split, including
after saving and reopening the project.

1. Open a fresh folder with `apiforge-desktop --folder PATH` and initialise it.
2. Create a scenario and replace its code with:

   ```afs
   scenario "QA Alpha" {
     alpha = 7
     beta = alpha + 3
   }
   ```

3. Wait for analysis and switch to Split or Diagram.
4. Save, close and reopen the project.

**Actual:** the saved checkpoint places `alpha` at `(0, 0)` and `beta` at
`(0, 120)`. The second card covers the lower part and ports of the first. The
overlap remains after restart.

**Expected:** new cards use their rendered dimensions for placement, so their
contents and ports remain visible without overlap.

**Impact:** even a two-step scenario produces a partially obscured diagram;
users must rearrange cards to read and connect them.

**Evidence:** [initial Split view](docs/qa/2026-10-24/p1-p4-native/split-good.png),
[Diagram after restart](docs/qa/2026-10-24/p1-p4-native/overlap-after-restart.png),
[full QA report](docs/qa/2026-10-24/p1-p4-native-interactive.md).

**Investigation:** `FolderApp::open` calls `ProjectSession::open`
(`crates/apiforge-desktop/src/folder/mod.rs:169`), selecting the default
`FixedPlacer`. Its uniform 240×96 card size
(`crates/apiforge-project/src/layout.rs:37`) does not match the native renderer's
larger, content-dependent cards. Supply a desktop placer using rendered card
dimensions for both opening and initialising folder projects. This is a separate
folder-session path from the legacy component-layout issue BUG-006.

## Coverage and limits

Continued QA on the same revision added BUG-011–017 and exercised inspector
URL conflicts, JSON body completion, normal restart with invalid field drafts,
main/cleanup navigation and whole-scenario Undo, an 80-block scenario, and
stale-draft conflict decisions.

| Area | UI verification / outcome |
| --- | --- |
| Opening Code, creating scenarios, pasting code | Tested; first edit loss in BUG-001 |
| Code → graph, parameters, references, assertions | Subsequent valid edits create/update blocks; four-step simulation with parameter quantity 2 passed |
| Graph/inspector → code | JSON body changed from A/2 to B/3; Open in code showed the updated values |
| Invalid scenario code | Error banner, underlines and disabled canvas appear; Run asks before running the last valid version; mutation bypass in BUG-002 |
| Completion from sample response shape | Correct alpha/beta suggestions with sample provenance; focus and positioning problems in BUG-003/010 |
| Document undo | Main-tab Undo restored an earlier value/assertion; cleanup-tab whole-code Undo restores only cleanup (BUG-017) |
| Scenario and project switching | Tested, including normal close/restart; cross-project persistence failure in BUG-008 |
| Draft restoration | Invalid draft for the last edited project restored correctly after restart; earlier project's draft did not |
| Component code | Valid additions, board layout and Save revision tested; BUG-006/007 |
| Immediate versus settled Run | Immediate run passed with old value 1; after synchronisation the same visible value 2 correctly failed. [Settled-run comparison](docs/qa/2026-10-24/BUG-009-current-run-fails.png) |
| Responsive layout and visual states | 1600 × 1000 and resize to 960 × 540; hidden-error disabled board; BUG-004/005/010 |
| Main/cleanup navigation and long code | Phase selection fails (BUG-014); Open in code leaves its target off screen in an 80-block scenario (BUG-015) |
| Stale draft versus external document update | Prompt appears after the stale value has already overwritten the document; Discard cannot retain the external value (BUG-016) |
| Inspector body/URL editing | Stale URL, mismatched completion syntax, and missing draft persistence (BUG-011–013); manual completion focus variant added to BUG-003 |

This was exploratory testing of the native **release binary through the UI**,
with source inspection to help locate confirmed failures. It was not a full test
suite run, accessibility audit, performance benchmark, or Windows/Wayland-native
validation. Runs used simulation; external API side effects were not exercised.
During the original exploratory QA, no fixes, commits or changes to the user's
existing workspace were made. The subsequent working-tree fixes are recorded in
the verification report linked at the top.
