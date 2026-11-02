# UX-1: the usability sprint — triage and proposed plan

- State: proposal of 2026-11-02, awaiting the owner's decisions (the quiz in
  [section 5](#5-owner-decisions)). Nothing here is scheduled until the owner
  answers; no issue was created, edited or labelled by this plan.
- Analysis model: Fable.
- Inputs: every open issue on 2026-11-02 (`gh issue list --state open`), read
  with its comments; the KTLO retrospective; UX-00 and the owner's five
  decisions of 2026-11-02; the code on `main` at `3dfc3f9` for the
  already-fixed check ([section 2](#2-duplicates-overlaps-and-what-is-already-fixed)).

The owner closed the MCP sprint on 2026-11-02 and asked for the next sprint
to go to the application's usability: "there are many issues; some were made
after tests in real work, so those problems must be solved." This document
sorts every open interface or usability issue, says which came from real
work, proposes the order, and lists the decisions the owner takes before any
ticket starts.

## How the sources were told apart

Every issue is filed from the owner's account, by the owner or by an agent.
The source was judged by content:

- **Real work** — found while using the application or the MCP server on a
  real project: the five KTLO findings (UX-01..05, raised after using the
  accepted KTLO changes), #193 (found through MCP-16's quiz), and the batch of
  2026-11-02 (#207–#218: "found in exploratory testing of the MCP server
  against a real project", "found with kwin-mcp … on a copy of a project",
  "found while writing scenarios for a JSON:API shop through MCP").
- **Owner wish** — a need the owner wrote after using the tool for API
  testing, without a defect report: #124's list (#125–#149), #192, #205.
- **Automated** — the Gemini Argon exploratory review of 2026-10-29
  (#150–#177), of which #150–#161 carry a second agent's verification
  comment (reproduced / partially / not reproduced) and #162–#177 are
  unverified; and review-bot findings (#202 from cubic, #201 as its
  follow-up).

Real-work findings come first in every wave.

## 1. Areas

Columns: issue · short title · source · size and owner review minutes · risk
flags (`visible UI`, `default`, `removal`, `format`) · mock-up. "Verified"
means the 2026-10-29 verification comment reproduced it; "code" means the
cited code is still on `main` and matches the report.

### 1.1 Header and run controls

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #99 UX-02 | Run mode is a setting; *Run simulation* (violet) / *Run live*; *Run options* becomes a chevron | real work, decided 2026-11-02 | S · 20 | visible UI, default | approved (UX-00) |
| #101 UX-04 | *Running unsaved code* moves to the header, only while the run runs | real work, decided 2026-11-02 | S · 15 | visible UI | approved (UX-00) |
| #152 | *Running saved/unsaved code* notice never clears | automated, verified | — | — | subsumed by UX-04 (the notice shows only while the run runs) |
| #165 (1, 2) | Component *Publish* stays enabled with no change; read-only banner flashes during save/publish | automated, code | S · 15 | visible UI, default | no (before/after capture) |
| #165 (3) | Publishing a component marks a scenario's fresh results stale | automated | in the same ticket | default | — |

### 1.2 Run results

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #98 UX-01 | Several checks from one run | real work, decided 2026-11-02 | M · 30 | visible UI | approved (UX-00) |
| #102 UX-05 | Run results and Problems drag to the whole workspace | real work, decided 2026-11-02 | S · 20 | visible UI | approved (UX-00) |
| #150 | A failed run neither selects nor scrolls to the failing step when a card is selected on the board | automated, verified | with #169 | default | with #169 |
| #169 | Nested components in Run results: no collapse, no auto-scroll, scrollbar over the duration, no breadcrumb | automated, code | M · 30 (one ticket with #150) | visible UI, default | yes |
| #160 (3, 4) | *Choose a field → add a check* with zero outputs; the summary omits *Skipped* | automated, code | in the wording ticket (1.12) | visible UI | no |
| #156 (2) | The hint's last letter is clipped beside a scrollbar | automated | in the wording ticket | — | no |
| #135 REP-01 | Component failure details at the scenario level | owner wish | M · 30 | visible UI | yes (with the #169 mock-up) |
| #148 REP-04 | JSON response view with a context menu for checks and variables | owner wish | M · 45 | visible UI | yes (RUN-00 group) |

### 1.3 Code editor

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #100 UX-03 | The red focus line turns grey; error marks appear after a pause in typing | real work, decided 2026-11-02 | S · 10 | visible UI, removal | approved (UX-00) |
| #216 KTLO-39 | Code completion opens inside a comment | real work | S · 15 (one ticket with #217) | — | no |
| #217 KTLO-40 | A click after the end of a line does not focus the editor | real work | with #216 | — | no |
| #193 | A code tab keeps its scroll across tab switches instead of re-centring the caret | real work (via MCP-16) | S · 15 | default | before/after capture |
| #176 (2) | The gutter's diagnostic dot overlaps two-digit line numbers; the dot is red for a warning | automated, code | S · 10 (one ticket with #153 (1)) | visible UI | no |
| #153 (1) | Ctrl+Z with invalid or pending source shows the raw *model is locked* text | automated, verified | with #176 (2) | — | no |
| #153 (2) | Which history takes Ctrl+Z in Code mode without editor focus | automated, contradicted by a test | — | default | decision 19; recommended: keep |
| #154 | Expanded Problems: lines cut through (unconfirmed); fills the ceiling for one error (approved KTLO-18 behaviour) | automated, partly disputed | S · 10 if reproduced | visible UI | decision 18 |
| #170 (5) | The completion popup overlaps the Problems header | automated | S · 5, with #176 (2) | — | no |
| #155 | *Apply fix* inserts `=` before unparseable input | automated, verified (parser) | S · 10 | — | no (LANG) |

### 1.4 Board and diagram

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #162 | Cleanup cards read *Missing step → …* for valid references to earlier Cleanup steps | automated, code | S · 10 | — | no |
| #163 | *Tidy up* does nothing when one card or a chip is selected | automated, code (and #168 (4)) | S · 10 | default | no |
| #171 | Selecting a card re-fits the board and zooms out across the detail threshold | automated, code | M · 20 (one ticket with #156 (1)) | default | before/after capture |
| #156 (1) | A check added from Run results lands under the Inspector | automated, unverified | with #171 | — | — |
| #164 | *Export* missing from *Add step* in components; *Start* offered; no *No matching steps* empty state; Export placed at x = 0 | automated, code | S · 15 | visible UI, removal (Start in components) | no |
| #166 (1) | A published component is offered inside itself | automated, code | S · 15 (one ticket with #166 (2)) | — | no |
| #175 (1, 2, 3) | *Update to revision N* fails when ports change; no update badge on cards; defaults shown as *Not set* | automated, code | M · 30 | visible UI | yes (badge) |
| #172 (3) | Card controls stay clickable under the Library panel | automated, code | S · 10 | — | no |
| #172 (1, 2) | Inline input does not select its card; invalid inline edits desynchronise from the Inspector | automated | M · 20 | default | yes (COMP-00 group) |
| #173 | Copy/paste limited to one graph tab; Duplicate overlaps and repeats labels; chips ignore Copy | automated, code | M · 20 | default | yes (COMP-00 group) |
| #174 | Component inputs cannot be edited in Diagram, are hidden by default, duplicate chips overlap | automated | L → split | visible UI, format (`@layout`) | yes (COMP-00 group) |
| #161 (1) | Card detail lost below 75 % zoom; failed cards hard to spot | automated, verified | M | visible UI | yes (DIAG-00 group) |
| #170 (2) | Anonymous `{ 2 fields }` pills at 50 %; URL subtitles clipped without an ellipsis | automated | with #161 (1) | visible UI | DIAG-00 group |
| #170 (1) | *Save as component* opens a modal saying it is not available in folder projects | automated | S · 10 | removal | decision 16 |
| #170 (3) | *Curves* wire style draws steps with rounded corners | automated, code | S · 5 | removal | decision 17 |
| #144 DIAG-01 | Descriptions beside blocks | owner wish | M | visible UI, format | yes (DIAG-00 group) |
| #145 DIAG-02 | Readable assertions on the diagram | owner wish | M | visible UI | yes (DIAG-00 group) |
| #167 | IF/ELSE emits a wire to every step of a branch; ports clash with left-to-right layout; no arrowheads | automated | L | visible UI, format | LAY (out) |
| #168 (1–3) | Proxy chips stranded by *Tidy up*; distort *Fit the view*; no collision avoidance | automated | M | default | LAY (out) |
| #170 (4) | Code → Diagram stacks new steps in one column | automated | — | — | duplicate of LAY-02 (#33) |

### 1.5 Inspector

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #147 UX-07 | Wider component properties panel on the right | owner wish | S · 15 | visible UI | yes (one capture per size) |
| #175 (3) | Optional inputs with a default read *Not set* | automated | in #175's ticket | visible UI | — |
| #161 (2) | Component inputs, description and action, and scenario settings, editable in Diagram | automated | L → split | visible UI | yes (COMP-00 group) |

### 1.6 Explorer and tabs

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #151, #176 (1) | The unsaved dot is truncated away on long tab names | automated, verified (two reports of one defect) | S · 10 | visible UI | no |
| #177 (1) | `apiforge.project.json` can be renamed, moved or deleted from the Explorer | automated, code | S · 10 (one ticket with #177 (2)) | visible UI, removal | decision 14 |
| #177 (2) | A text file opened in Diagram has no *Show code* button | automated | with #177 (1) | visible UI | decision 15 |
| #166 (2) | *New component* lands in `scenarios/` when a scenario is selected | automated | with #166 (1) | default | decision 13 |
| #146 UX-06 | Shared left sidebar; the Explorer available in Split | owner wish | M · 30 | visible UI, default | yes (WS-00 group) |
| #161 (4) | Four columns at 1600 × 1000 in Split squeeze the diagram to 34 % | automated, verified | with UX-06 | default | WS-00 group |
| #157 | Cards and the *Add step* popup clipped at 960 × 540 with panels open | automated, partly reproduced | M · 20 | default | WS-00 group |

### 1.7 Dialogs

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #214 MCP-26 | *Cancel* in Project settings leaves the agent's live permission on; the sentence understates the reach | real work · **critical** (consent) | S · 15 | visible UI, default | before/after capture |
| #202 KTLO-36 | *Extract component* proposes *Required* ticked for an argument that gets a default | automated (cubic), owner requested | S · 10 | visible UI, default | before/after capture |
| #160 (2) | The JSON hint on an Object or List field suggests `"abc"` | automated, code | in the wording ticket | — | no |
| #158 | *Data contracts* tab clipped to one line | automated, **not reproduced** | — | — | stays in the backlog until reproduced |
| #161 (3) | Run inputs: fill from the contract sample; *Run with inputs…* | automated | S–M | visible UI | yes (RUN-00 group) |
| #161 (5) | *Apply settings* then *Save all*; no *Discard changes* on a tab | automated, intentional today | M | default | RUN-00 group |

### 1.8 Bottom bar and window

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #215 KTLO-38 | The first window opens taller than the screen and hides the holder bar | real work · **critical** (the person cannot see *Take back*) | S · 15 | default | no (a capture at 1600 × 1000 with decorations) |
| #192 MCP-20 | A command's live run shows its status in the observing window's bar | owner wish | S · 15 | visible UI | yes |
| #213 MCP-25 | *Take back* stops an agent only until its next write | real work, `decision-needed` | S–M · 20 | default, visible UI | decision pending on the issue; not critical by the owner's definition |

### 1.9 Notices

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #218 KTLO-41 | *Saved N files.* never clears | real work | S · 10 (one ticket with #175 (4)) | default | no |
| #175 (4) | The red error banner persists across tabs | automated | with #218 | default | no |
| #152 | *Running …* never clears | automated, verified | — | — | subsumed by UX-04 |

### 1.10 Onboarding and empty states

No real-work finding. Two empty states ride on other tickets: *No matching
steps* in the *Add step* search (#164) and *Show code* under the text-file
message in Diagram (#177 (2)).

### 1.11 Keyboard and accessibility

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #159 | Duplicate accessible labels: report rows, tab vs Explorer, *Run* under the modal, contract rows | automated, partly confirmed | S · 10 | — | no |
| #153 | Ctrl+Z / Ctrl+Y | automated | see 1.3 | | |
| #173 | Ctrl+C / Ctrl+V across graphs | automated | see 1.4 | | |

### 1.12 Wording and polish (one ticket)

| Issue | Title | Source | Size | Risk | Mock-up |
| --- | --- | --- | --- | --- | --- |
| #160 (1, 5) | *1 values*, *1 fields*, *1 items*; *Saving them is paused* for one file | automated, code | S · 10 for the whole ticket | — | no |
| #160 (2, 3, 4) | JSON hint by type; the add-check hint only with outputs; *Skipped* in the summary | automated, code | in the same ticket | visible UI | no |
| #156 (2) | The clipped hint | automated | in the same ticket | — | no |

Parts 1 and 5 contain no decision and could go to Sonnet alone; parts 2–4
and the hint make it an Opus ticket. One ticket keeps the review to one
sitting.

### 1.13 MCP issues — critical or not

The owner's definition: critical blocks the agent's work, loses or corrupts
data, leaks a secret or sends a live request without consent, crashes the
window or the server, or gives misleading answers that steer the model
wrong. Critical ones join the sprint; the rest stay in the backlog.

| Issue | Verdict | Why |
| --- | --- | --- |
| #214 MCP-26 | **critical** | A permission ticked by mistake and cancelled stays granted: live requests without consent. Dialog area, S · 15. |
| #208 MCP-22 | **critical** | Three fields say something other than what happened (`may_have_reached_api` false on a request that got a response; a replayed publish says `wrote: true`; "asked when the run starts" when nobody is asked). Misleading answers that steer the model. Non-UI, S · 10, merge on green CI under the standing authorisation. |
| #207 MCP-21 | not critical | `used` is noise that grows with every publish; it misdescribes but does not steer an action. Backlog; S when taken. |
| #209 MCP-23 | not critical | An enhancement (export values in the compact result). Backlog. |
| #210 MCP-24 | not critical | A `format` decision about the manifest, `decision-needed`. Backlog; the decision is asked on the issue, not here. Note: the agent wrote seed passwords as input defaults because it had no way to declare a secret — the leak is the agent's, not the server's, but it argues for option A or B there. |
| #213 MCP-25 | not critical | *Take back* holds only until the agent's next write; the window is clean when it hands over again, so nothing is lost. A `default` decision pending on the issue. Backlog, candidate for UX-2. |
| #211 KTLO-37 | not critical | `print` and `fmt` disagree on attribute order; files churn on save but are not corrupted. `format` decision; backlog. |
| #212 LANG-07 | not critical | Language feature. Backlog. |
| #219 KTLO-42 | not critical | Development tooling: the egui MCP server's relative path. Outside the sprint's capacity as a Sonnet fix (S · 5, no decision). |
| #191 MCP-19 | in flight | PR #203 awaits acceptance; non-UI, merge on green CI authorised on the issue. |
| #192 MCP-20 | not critical | Owner wish; bottom bar; S · 15 with a mock-up. Wave 4 or backlog (decision 2). |
| #95 MCP-11 | leftover | The sprint's acceptance ticket is open while the sprint is closed (decision 3). |
| #205 REL-04 | in flight | Draft PR #206, decisions taken 2026-11-02; owner chose "before v0.1.0". S · 15; counts against the WIP cap when it is ready. |
| #122 KTLO-33 | in flight | Draft PR #204 (analysis of the CI flakes). Non-UI. |

### Counts

| Area | Issues (parts counted once per issue) | Of which real work |
| --- | --- | --- |
| Header and run controls | 4 (#99, #101, #152, #165) | 2 |
| Run results | 8 (#98, #102, #150, #169, #160, #156, #135, #148) | 2 |
| Code editor | 9 (#100, #216, #217, #193, #176, #153, #154, #170, #155) | 4 |
| Board and diagram | 16 (#162, #163, #171, #156, #164, #166, #175, #172, #173, #174, #161, #170, #144, #145, #167, #168; #170 (4) duplicates LAY-02 #33) | 0 (2 owner wishes) |
| Inspector | 3 (#147, #175, #161) | 0 (1 owner wish) |
| Explorer and tabs | 7 (#151, #176, #177, #166, #146, #161, #157) | 0 (1 owner wish) |
| Dialogs | 6 (#214, #202, #160, #158, #161) | 1 |
| Bottom bar and window | 3 (#215, #192, #213) | 2 |
| Notices | 3 (#218, #175, #152) | 1 |
| Keyboard and accessibility | 1 own (#159) | 0 |
| Wording and polish | 1 ticket (#160, #156) | 0 |
| MCP and leftovers | 14 (#207–#214, #219, #191, #192, #95, #205, #122) | 8 |

Several issues appear in two areas because they bundle unrelated parts
(#156, #160, #161, #165, #166, #170, #175, #176); the ticket column says
where each part lands.

## 2. Duplicates, overlaps and what is already fixed

### Duplicates and merges

| Keep | Merge in | Why |
| --- | --- | --- |
| #151 | #176 (1) | The same tab-dot truncation, reported twice (phase 1 and phase 3). |
| UX-04 (#101) | #152 | UX-04's decision: the notice shows only while the run runs, in the header. Nothing is left of #152. |
| #218 | #175 (4) | One ticket for the lifecycle of non-error notices and of the error banner. |
| #169 | #150 | #169 (2) is #150's scroll; the selection policy and the collapsible rows are one Run results navigation design. REP-01 (#135) shares the mock-up. |
| #171 | #156 (1) | Both are the camera reacting to the Inspector's width: a card is hidden or the whole board zooms out. One fix in `canvas/mod.rs`. |
| #163 | #168 (4) | A selected chip makes `tidy_up` return `None` the same way one card does. |
| #216 | #217 | Two S editor defects from the same real-work session, one PR. |
| #176 (2) | #153 (1), #170 (5) | Editor gutter and popup rendering, after UX-03. |
| #166 (1) | #166 (2) | One ticket: component creation and self-reference. |
| #177 (1) | #177 (2) | One ticket: non-`.afs` files in the Explorer. |
| #160 | #156 (2) | Wording and polish. |
| LAY-02 (#33) | #170 (4) | Code → Diagram stacking is LAY-02's goal. |
| UX-19 | backlog *Later* item "Run results does not scroll to the selected step and cuts the fifth step off" | The same defect, noted at the KTLO-23 mock-up before #150 reported it. |
| LAY line | #167, #168 (1–3) | Routing and chip placement belong to the layout analysis (LAY-00), not to UX-1. |
| COMP-01 (#137) | #175 (1, 2) | The owner's wish (managing revision updates in calls) and the automated finding (*Update to revision* fails, no badge) are the same workflow. #175's defect part is in UX-1; COMP-01's design stays in the backlog. |
| ENV-01 (#138) | #210 MCP-24 | Both concern where a base URL and variables come from; decided together later. |

### Conflicts with an approved UX-01..05 decision

- **#154 (2)** proposes a content-dependent height for the expanded
  Problems panel. KTLO-18, approved and accepted, fills to the ceiling minus
  a strip of code, and UX-05 keeps that strip when dragged to the top. Taking
  #154 (2) would reopen KTLO-18 (decision 18; recommended: keep).
- **#165 (3)** asks that publishing a component not mark a scenario's fresh
  results stale. UX-01 decided the panel bar's wording: *N checks added* when
  only checks were added, *Graph changed* for any other change. The rule for
  "a called component was republished" must fit that wording; it is in
  #165's ticket, after UX-01.
- **#161 (3)** suggests a *Run with inputs…* item in *Run options*. UX-02
  turns *Run options* into a chevron menu with the mode and removes the
  one-off *Run live requests* item; a new item there is a change to the
  approved menu and waits for the RUN-00 mock-up.
- **#157** suggests folding panels automatically at 960 × 540; UX-05 adds a
  drag-to-top gesture for the same panels. Workspace allocation is one
  mock-up (WS-00), after UX-05.
- **#152** is not a conflict: UX-04 resolves it.

### Already fixed on `main`?

Checked on `3dfc3f9` (2026-11-02). `git log adb0a80..main --
crates/apiforge-desktop/src` lists only MCP-08c, MCP-09, MCP-10, MCP-12,
MCP-15, MCP-16, REL-02 and KTLO-32; none of them touched the sites the
findings cite. Each cited site was then read on `main`:

| Finding | Site on `main` | Result |
| --- | --- | --- |
| #151 / #176 (1) | `folder/chrome.rs:242–248`: the dot is appended before `truncate_at_width(TAB_LABEL_WIDTH)` | not fixed |
| #152 / #218 | `folder/mod.rs:212` `Notice { text, error }` has no lifetime; `runs.rs:268` sets it, `saving.rs:198` sets *Saved N files*, nothing clears a non-error notice | not fixed |
| #153 | `folder/mod.rs:1219–1221`: `step_model_history` runs before the `Mode::Code` guard; the raw `ModelLocked` text reaches the banner | not fixed (part 2 is deliberate, tested) |
| #154 | `folder/code_view.rs:45–70`: `exact_size(ceiling)` as KTLO-18 specified | by design; clipping unconfirmed |
| #155 | `apiforge-script/src/parser.rs:121`: the insertion fix does not look at the next token | not fixed |
| #156 (1) | `folder/runs.rs` `AssertField`: no `reveal` | not fixed |
| #160 | `labels.rs:63–78` plural always; `folder/mod.rs:1142` *Saving them*; `report.rs:640` hint without an outputs check; the summary counts Passed and Failed only | not fixed |
| #162 | `folder/diagram.rs:119, 208, 436`: `reference = graph_of(&model, false)` for the Cleanup board too | not fixed |
| #163 | `board_edit.rs:64–68`: `count() < 2` returns `None` | not fixed |
| #164 | `canvas/menu.rs:77–94`: no `Block::Export`; `Block::Start` listed | not fixed |
| #166 | `canvas/menu.rs:110` and `library.rs` iterate `latest_components(project)` without excluding the active document | not fixed |
| #171 | `canvas/mod.rs:376–381`: `fit` on a board size change while `!camera_moved_by_user` | not fixed |
| #173 | `folder/mod.rs:1552` `context_key(document_id, cleanup)` binds the clipboard to one graph | not fixed |
| #175 | `inspector/mod.rs:266`: `node.config.revision = Some(newest)` only | not fixed |
| #176 (2) | `code.rs:412–418` gutter width without room for the dot; `code.rs:550` the dot is `theme::DANGER` for every severity (the row tint at `code.rs:1000` already uses `WARNING`, as it did at the baseline) | not fixed; the report's "warnings in red" applies to the dot only |
| #177 | `folder/explorer.rs:130` every file row is `portable: true`; `:544` enables Rename/Delete for any portable row | not fixed |
| #193 | `code.rs:181–190`: the prepared caret is scrolled to `Align::Center` unless `gentle` (MCP-16's follow only) | not fixed |
| #158 | — | never reproduced by the verifier; no change on `main` |
| #216, #217, #215, #214, #208 | reported on 2026-11-02 against the current build | open |

Nothing in the list is fixed. The verification comments of 2026-10-29
remain the latest word on #150–#161; #162–#177 were checked against the code
only, not reproduced at the window.

## 3. Proposed order — waves

Rules applied: UX-01..05 and the critical real-work problems first; at most
four pull requests awaiting acceptance within 60 review minutes; no two
tickets on the same screen region in flight at once (header, Run results
panel, editor gutter, board camera, Explorer, a dialog); the `after` column
sequences them. Minutes are owner review minutes.

Proposed ticket IDs continue the UX line; the caller creates the issues after
the quiz.

### Wave 1 — the decided findings and the critical real-work problems (135 min)

| ID | Issue(s) | Result | Size | Region | After |
| --- | --- | --- | --- | --- | --- |
| UX-01 | #98 | Several checks from one run | M · 30 | Run results | — |
| UX-02 | #99 | Run mode setting; *Run simulation* / *Run live*; chevron | S · 20 | header | — |
| UX-03 | #100 | Grey focus line; error marks after a pause | S · 10 | editor gutter | — |
| UX-04 | #101 (+#152) | *Running unsaved …* in the header, only while running | S · 15 | header | UX-02 |
| UX-05 | #102 | Run results and Problems drag to the whole workspace | S · 20 | Run results, Problems | UX-01 |
| UX-08 | #214 MCP-26 | *Cancel* in Project settings restores the live toggle; the sentence states the reach | S · 15 | settings dialog | — (decision 4) |
| UX-09 | #215 KTLO-38 | The first window fits the screen's work area | S · 15 | window | — (decision 5) |
| UX-10 | #208 MCP-22 | Three MCP result fields say what happened | S · 10 | MCP (non-UI) | — |

Batches within the cap: (a) UX-01, UX-02, UX-03 = 60 min; (b) UX-04, UX-05,
UX-08, UX-10 = 60 min; (c) UX-09 with the start of wave 2. UX-10 is non-UI
and may merge on green CI under the standing authorisation, which frees its
slot early.

### Wave 2 — real-work editor and notice defects, and the verified defects without a design (140 min)

| ID | Issue(s) | Result | Size | Region | After |
| --- | --- | --- | --- | --- | --- |
| UX-11 | #216, #217 | No completion in a comment; a click after a line's end focuses the editor | S · 15 | editor | UX-03 |
| UX-12 | #218, #175 (4) | Non-error notices clear; the error banner is per document | S · 10 | notice bar | UX-04 (decision 6) |
| UX-13 | #193 | A tab keeps its scroll; the caret is restored without re-centring | S · 15 | editor | UX-11 (decision 7) |
| UX-14 | #202 KTLO-36 | *Extract component*: *Required* unticked when the argument gets a default | S · 10 | dialog | — (decision 8) |
| UX-15 | #151, #176 (1) | The unsaved dot survives a truncated tab name | S · 10 | tab strip | — |
| UX-16 | #162 | Cleanup cards resolve references to Cleanup steps | S · 10 | board cards | — |
| UX-17 | #163, #168 (4) | *Tidy up* with one card or a chip selected tidies the diagram | S · 10 | board toolbar | — (decision 9) |
| UX-18 | #171, #156 (1) | The Inspector opening does not re-fit the board; the selected or new card stays visible | M · 20 | board camera | — |
| UX-19 | #150, #169 | Run results navigation: scroll to the selected row, selection on a failed run, collapsible component rows, breadcrumb, scrollbar padding | M · 30 | Run results | UX-05 (decisions 10, 11; mock-up) |
| UX-20 | #176 (2), #153 (1), #170 (5) | Gutter room for the dot, amber for warnings; a plain message for undo on invalid source; the popup clears the Problems header | S · 10 | editor gutter | UX-13 |

### Wave 3 — components, Explorer, dialogs and polish (140 min)

| ID | Issue(s) | Result | Size | Region | After |
| --- | --- | --- | --- | --- | --- |
| UX-21 | #164 | *Export* in *Add step* inside components; *Start* not offered there; *No matching steps*; Export placed at the sink | S · 15 | Add step menu | — (decision 12) |
| UX-22 | #166 | A component is not offered inside itself; *New component* lands in `components/` | S · 15 | library, Explorer | UX-15 (decision 13) |
| UX-23 | #177 | The manifest is protected in the Explorer; a text file in Diagram offers *Show code* | S · 10 | Explorer | UX-22 (decisions 14, 15) |
| UX-24 | #175 (1–3) | *Update to revision N* succeeds when ports change and leaves the gaps as diagnostics; an update badge on the card; defaults shown | M · 30 | inspector, board cards | UX-18 (mock-up for the badge) |
| UX-25 | #165 | *Publish* disabled with nothing to publish; no banner flash; the stale-results rule for a republished component | S · 15 | header, Run results bar | UX-04, UX-19 (decision 16) |
| UX-26 | #160, #156 (2) | Wording, plurals, hints, *Skipped* in the summary | S · 10 | several, text only | UX-19 |
| UX-27 | #159 | Distinct accessible labels | S · 10 | several, labels only | UX-26 |
| UX-28 | #170 (1), #170 (3) | *Save as component* disabled with a reason in folder projects; *Curves* removed until it draws curves | S · 10 | board toolbar | UX-17 (decisions 17, 18) |
| UX-07 | #147 | A wider Inspector | S · 15 | inspector | UX-24 (one capture per size) |
| UX-29 | #172 (3) | Card controls clipped to the board | S · 10 | board | UX-18 |

### Wave 4 — the shared mock-ups (owner wishes and designs; not estimated beyond the mock-up)

Each group is one analysis ticket with one mock-up, as UX-00 was, and its
tickets are estimated on the mock-up. Review: about 30 min per mock-up plus
the discussion.

| Group | Issues | Mock-up shows |
| --- | --- | --- |
| WS-00 workspace | UX-06 #146, #161 (4), #157 | One left sidebar (Files / Library) in Code, Diagram and Split; what folds at 960 × 540 with Run results and the Inspector open; the *Add step* popup at 540 px |
| DIAG-00 card readability | DIAG-01 #144, DIAG-02 #145, #161 (1), #170 (2) | A card at 50 %, 62 % and 91 %: labels, the failed state, a description beside a block, a readable check; the `format` question of where a description lives |
| COMP-00 component authoring in Diagram | #174, #161 (2), #173, #172 (1, 2) | Inputs on the component board; the component's metadata without Code mode; copy and paste across graphs; inline edits and the Inspector in step |
| RUN-00 run inputs and settings | #161 (3), #161 (5), REP-04 #148 | *Run with inputs…*, the sample from a contract, settings saved in one step, a *Discard* action, the JSON response view with its menu |
| MCP-20 | #192 | A command's run in the bottom bar (its own S mock-up) |

REP-01 (#135) shares UX-19's mock-up and becomes a ticket when that mock-up
is decided.

### Totals

| Wave | Tickets | Review minutes |
| --- | --- | --- |
| 1 | 8 | 135 |
| 2 | 10 | 140 |
| 3 | 10 | 140 |
| 1–3 | 28 | **415** |
| 4 | 5 mock-ups | about 150 for the mock-ups; tickets estimated afterwards |

At the MCP sprint's pace (about 60 review minutes a day) waves 1–3 take about
two weeks of the owner's reviews; the WIP cap keeps four tickets in flight.

## 4. Out of the sprint

| Issues | Why |
| --- | --- |
| LAY-01..06 (#32–#37), #167, #168 (1–3), #170 (4) | Layout and wire routing: the LAY line has its own analysis (LAY-00) and the owner's decision of 2026-10-27 put it after MCP; decision 2 says whether UX-1 goes first. |
| #125–#143 (DB, RUN, LANG, HTTP, REP-02/03, INT, SIM, COMP-01, ENV-01) | Functional gaps of the language, runner and reports — the owner's items 1–5 are here. They need their own sprint with language decisions (`format`); none is an interface defect. COMP-01 and ENV-01 are noted against #175 and #210. |
| #124 BACKLOG-01 | A tracking issue. |
| #207, #209, #210, #213, #211, #212 | MCP and language issues that are not critical by the owner's definition (section 1.13). #210 and #213 carry their own `decision-needed`. |
| #219 KTLO-42 | Tooling; a Sonnet fix outside the sprint's capacity (no owner decision, 5 min). |
| #155 | A parser quick-fix quality rule (LANG), verified but not a window defect; S · 10 when a LANG sprint runs. |
| #158 | Not reproduced; stays until someone reproduces it with the revision and the sequence. |
| #116 BRAND-00 | After LAY by the owner's note. |
| #191 MCP-19, #205 REL-04, #122 KTLO-33 | In flight from the MCP sprint; they finish under their own authorisations and count against the WIP cap while open. |
| #95 MCP-11 | Decision 3. |
| #201 KTLO-35 | Docs only, authorised to merge on green CI; not a sprint ticket. |

## 5. Owner decisions

Each question lists the recommended option first. Answers are recorded on
the issues by the coordinating session after the quiz.

**Sprint shape**

1. **Capacity of UX-1.**
   A. Waves 1–3: 28 tickets, 415 review minutes, about two weeks; wave 4's
   mock-ups follow as UX-2.
   B. Waves 1–2: 18 tickets, 275 minutes.
   C. Waves 1–4: waves 1–3 plus the five mock-ups (about 565 minutes) with
   their tickets estimated during the sprint.
   D. Wave 1 only: 8 tickets, 135 minutes, then re-plan.

2. **UX-1 and LAY.** On 2026-10-27 LAY was to follow MCP.
   A. UX-1 runs now; LAY follows UX-1 (the owner's words of 2026-11-02).
   B. LAY-01..03 (40 minutes, no mock-up) join UX-1's wave 3; LAY-04..06
   follow UX-1.
   C. LAY first, UX-1 after it.

3. **MCP-11 (#95), the closed sprint's acceptance ticket.**
   A. Close it on the record of the 2026-11-02 session: the client's version
   and the steps go into `docs/tickets/MCP-11.md` as its report, with the
   defects it found (#207–#218) named as the result.
   B. Keep it open and run the scripted end-to-end once more after UX-1.
   C. Close it without a report.

**Wave 1**

4. **#214 — the live toggle in Project settings (critical).**
   A. The toggle takes effect on *Apply settings* only; *Cancel* discards it;
   the sentence reads "the agent may send real requests to any URL the
   project's documents name".
   B. Immediate effect stays; the button is renamed *Close* and the sentence
   under the toggle says the change is immediate and names the reach.
   C. Immediate effect stays; *Cancel* restores the state the dialog opened
   with.

5. **#215 — the first window's size (default).**
   A. The first window is the smaller of 1600 × 1000 and the monitor's work
   area minus the title bar, never below 960 × 540; a remembered size is
   clamped the same way.
   B. 1600 × 964 so that a 36 px title bar fits a 1000 px screen.
   C. The first window opens maximised.

**Wave 2**

6. **#218 — transient notices (default).**
   A. A non-error notice (*Saved N files.*, *Published revision N.*) clears
   after five seconds or on the next action; errors stay until dismissed;
   the red banner belongs to the tab that raised it.
   B. Notices stay until dismissed (today); only the error banner becomes
   per tab.
   C. Notices clear on the next action only, never by time.

7. **#193 — a tab's scroll (default).**
   A. A tab keeps its scroll; the caret is restored without moving the view
   unless it is outside the view, then the view moves the least that shows
   it.
   B. As today: the view re-centres on the caret at every switch.
   C. The scroll is kept even when the caret is out of view.

8. **#202 — *Extract component*'s *Required* box (default).**
   A. Unticked when the extracted literal becomes the argument's default
   (the runner's rule since KTLO-32).
   B. Ticked, as today, which contradicts the default the dialog writes.

9. **#163 — *Tidy up* with a selection (default).**
   A. With nothing, one card or a chip selected, *Tidy up* tidies the whole
   board; with two or more cards it tidies the selection.
   B. As today, but the button is disabled with "select two or more cards or
   nothing" when nothing would move.

10. **#150 — Run results after a failed run (default).**
    A. A run that failed selects its first failed, blocked or errored step
    and scrolls to it, even when a card is selected on the board; a passing
    run keeps the board's selection.
    B. The board's selection wins (today); the list only scrolls to it.

11. **#169 — component rows in Run results (visible UI).**
    A. A component's rows collapse when all its steps passed and open when
    one did not; a breadcrumb names the path of a nested step.
    B. Always expanded (today), with the breadcrumb only.

**Wave 3**

12. **#164 — *Start* and *Data condition* inside a component (removal).**
    A. Not offered inside a component; *Export* is; the popup's subtitle
    names the component.
    B. Everything stays offered everywhere; only *Export* is added.

13. **#166 — where *New component* lands (default).**
    A. *New component* always creates in `components/`, *New scenario* in
    `scenarios/`, whatever row is selected.
    B. The selected folder (today).

14. **#177 — `apiforge.project.json` in the Explorer (removal).**
    A. Shown, protected: no *Open as text*, *Rename*, *Move* or *Delete*;
    clicking it explains what it is without an error banner.
    B. Hidden from the Explorer.
    C. As today.

15. **#177 — a text file opened in Diagram.**
    A. A *Show code* button under the message, as other empty states have.
    B. The view switches to Code by itself.

16. **#165 — *Publish* with nothing to publish (default).**
    A. *Publish* is disabled and reads the latest revision when the component
    equals it; republishing a component marks a scenario's results stale only
    when the scenario calls that component.
    B. *Publish* stays enabled (today); only the banner flash is fixed.

17. **#170 — *Save as component* in folder projects (removal).**
    A. Disabled with "not available in folder projects yet" on hover, until
    it is implemented.
    B. The modal stays (today).
    C. Implement it (a ticket of its own, M, after COMP-00).

18. **#170 — the *Curves* wire style (removal).**
    A. Removed from the menu until it draws curves; a saved `curves`
    preference falls back to *Steps*.
    B. Kept as it is.
    C. Implemented as real curves (S–M, wave 4).

**Not recommended, asked for completeness**

19. **#154 — the expanded Problems panel.**
    A. Keep KTLO-18: it fills to the ceiling minus a strip of code; the line
    clipping is fixed only if a reproduction shows it.
    B. The panel grows with its content (reopens KTLO-18's mock-up).

20. **#153 — Ctrl+Z in Code mode without editor focus.**
    A. Keep: the model's history takes it, as the test
    `model_history_survives_switches_and_ctrl_z_follows_the_focus` fixes.
    B. Ignore Ctrl+Z when the editor has no focus.

## Technical appendix

- Code sites named above are on `main` at `3dfc3f9`; line numbers move.
- Every wave-2 and wave-3 ticket with a `default` or `removal` flag gets a
  ticket file under `docs/tickets/` with the decision recorded before code,
  as the process requires; S tickets without a flag may be issues only.
- Tickets without a decision and with named tests (UX-10, UX-11, UX-15,
  UX-16, UX-26, UX-27, UX-29) are candidates for `ready:sol` once their
  analysis closes; #160 (1, 5) alone would be `ready:sonnet`.
- The verification comments on #150–#161 named the tests that pin current
  behaviour (`resolve_selected_prefers_explicit_then_board_then_failed`,
  `model_history_survives_switches_and_ctrl_z_follows_the_focus`,
  `ktlo23b_the_entered_values_run_and_are_kept_for_the_component`); a ticket
  that changes one of them says so in its acceptance tests.
