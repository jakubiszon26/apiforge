# Backlog

Tickets are [GitHub issues](https://github.com/jakubiszon26/apiforge/issues),
one per ticket, titled `<ID>: <goal>` and labelled as the
[development process](process/README.md#labels) says. Their state — open,
closed, `decision-needed`, `awaiting-acceptance` — lives there and nowhere
else. This file names the current sprint, its exit criteria and the issues
in it, and keeps the record of finished sprints.

At most four pull requests await owner acceptance at a time, within 60
estimated review minutes ([limits](process/README.md#limits-that-keep-review-possible)).
A ticked exit criterion names the evidence that ticked it: the report, the
run or the issue.

## Sprint: KTLO (2026-10-25 to 2026-10-27, closed)

Keep the lights on: stabilise what Stage II shipped and run the new process
end to end once. Planned for 2026-10-26 to 2026-11-08 and closed by the owner
on 2026-10-27 with every ticket accepted; the
[retrospective](retrospectives/2026-10-27-ktlo.md) has the facts and the three
actions. No new features, with one exception the owner made on 2026-10-26:
five interface wishes for the folder window (KTLO-17, KTLO-11, KTLO-18,
KTLO-19, KTLO-20) joined the sprint and went through the full path, mock-up →
decision → acceptance tests → QA report, about 115 minutes of owner review. A
sixth wish was added later the same day: running a component from its own tab
(KTLO-23, two parts, 65 further owner minutes), and a seventh, KTLO-24 (#57,
S · 15 min, the open-files bar hides itself). The sprint had no capacity line
for additions; the MCP sprint has one. Exit criteria:

- [x] Migration of the owner's real workspace works (KTLO-03, #5; the
      six-project workspace migrates, `docs/qa/2026-10-27/KTLO-03/report.md`,
      merged in #78 and #83).
- [x] No ignored test without a ticket; the identity checkpoint decision is
      made (KTLO-04, #6, merged in #74). One Stage II ignore remains without
      a ticket (`timing; run in release`, `folder/tests.rs`) and 24 review
      views are a named exception since KTLO-28; the former gets its ticket at
      MCP planning.
- [x] The block library in the folder mode is accepted by the owner through
      the full process: mock-up, acceptance tests, QA report (KTLO-05a #7,
      KTLO-05b #8; accepted 2026-10-26). Follow-ups: KTLO-05c #22,
      KTLO-05d #23, both merged.
- [x] The process documents were followed once end to end, and amended where
      the pilot required it (#15 tickets as issues; KTLO-28, 29, 30 from the
      retrospective).
- [ ] The regression smoke list has run on Linux and on Windows (KTLO-07,
      #10). **Linux only**: `docs/qa/2026-10-26/KTLO-07/report.md` records
      every Windows item as pending, and no later report ran it. Carried into
      MCP: the owner's local build or the Windows CI captures of the first MCP
      ticket that touches the window.
- [x] The two interface findings the owner raised on the mock-ups — one header
      above the board (KTLO-12, #16) and an editor without a box (KTLO-13,
      #17) — are accepted through the same mock-up → test → report path
      (KTLO-13 merged in #39, KTLO-12 in #41 and #47).
- [x] The five interface wishes of 2026-10-26 are decided on their mock-ups
      and accepted (KTLO-17 #48, KTLO-11 #77, KTLO-18 #71, KTLO-19 #69,
      KTLO-20 #52).
- [x] A component runs from its tab the way a scenario does (KTLO-23a #75,
      KTLO-23b #80; the optional-input scenario the criteria missed, KTLO-27
      #85).
- [x] The open-files bar hides itself when no file is open (KTLO-24, #70).
- [x] The KTLO retrospective is written and its actions are issues (KTLO-10
      #13; actions KTLO-28 #86, KTLO-29 #87, KTLO-30 #88, delivered in the
      retrospective's pull request).

| Issue | Ticket | Estimate |
| --- | --- | --- |
| — | KTLO-01 Process documents — merged in #1 | M (docs) · 30 min; estimated S, ~730 lines written |
| — | KTLO-02 Stage II retrospective — merged in #1; facts corrected in #2 | S (docs) · 15 min |
| [#5](https://github.com/jakubiszon26/apiforge/issues/5) | KTLO-03 Migration refuses a real six-project workspace; [ticket](tickets/KTLO-03.md); after KTLO-04 | M · 30 min · `migration`, `visible UI` |
| [#6](https://github.com/jakubiszon26/apiforge/issues/6) | KTLO-04 Identity checkpoint at initialise, register_existing and import; [ticket](tickets/KTLO-04.md) | S–M · 30 min · `format` |
| [#7](https://github.com/jakubiszon26/apiforge/issues/7) | KTLO-05a Block library — analysis and mock-up; [ticket](tickets/KTLO-05a.md) | S · 20 min · `visible UI`; actual 120 lines, 1 round |
| [#8](https://github.com/jakubiszon26/apiforge/issues/8) | KTLO-05b Block library — implementation (pilot of the full procedure); [ticket](tickets/KTLO-05b.md) — merged in #21 | M · 45 min · `visible UI`; actual 788 lines, 2 rounds |
| [#22](https://github.com/jakubiszon26/apiforge/issues/22) | KTLO-05c The library panel's width is remembered per project (owner decision: unknown panel sizes are ignored) | S · 10 min · `format` |
| [#25](https://github.com/jakubiszon26/apiforge/issues/25) | KTLO-15 A foreign write to a scenario file is sometimes not seen as a conflict on Windows | S · 10 min |
| [#23](https://github.com/jakubiszon26/apiforge/issues/23) | KTLO-05d The Add step popup lists Variables and Operators below 1440 px | S · 15 min · `visible UI` |
| [#9](https://github.com/jakubiszon26/apiforge/issues/9) | KTLO-06 Legacy JSON mode sunset: parity list and deprecation notice | S (docs) · 20 min · `removal` |
| [#10](https://github.com/jakubiszon26/apiforge/issues/10) | KTLO-07 Regression smoke list: first run on Linux and Windows | S · 45 min |
| [#11](https://github.com/jakubiszon26/apiforge/issues/11) | KTLO-08 CI uploads QA captures per platform; [ticket](tickets/KTLO-08.md) — merged in #4 | S · 10 min; actual 483 lines (M), 2 rounds |
| [#12](https://github.com/jakubiszon26/apiforge/issues/12) | KTLO-09 Orphaned code audit; [ticket](tickets/KTLO-09.md) — merged in #20 | S · 10 min; actual 118 lines, 2 rounds |
| [#13](https://github.com/jakubiszon26/apiforge/issues/13) | KTLO-10 KTLO retrospective | S (docs) · 30 min |
| [#16](https://github.com/jakubiszon26/apiforge/issues/16) | KTLO-12 One header above the board: title and Main/Cleanup beside Add step | S · 15 min · `visible UI` |
| [#17](https://github.com/jakubiszon26/apiforge/issues/17) | KTLO-13 The code editor fills its panel without a bordered box | S · 15 min · `visible UI` |
| [#18](https://github.com/jakubiszon26/apiforge/issues/18) | KTLO-14 CI has failed on every push to main since Stage II began; [ticket](tickets/KTLO-14.md) — merged in #19 | S · 10 min; actual 195 lines, 4 rounds |
| [#31](https://github.com/jakubiszon26/apiforge/issues/31) | KTLO-16 The Linux folder-projects CI step sometimes hangs; [ticket](tickets/KTLO-16.md) — merged in #40 | S · 10 min |
| [#42](https://github.com/jakubiszon26/apiforge/issues/42) | KTLO-17 The block library collapses to a rail in Diagram and Split; [ticket](tickets/KTLO-17.md) | S · 20 min · `visible UI`, `format` |
| [#14](https://github.com/jakubiszon26/apiforge/issues/14) | KTLO-11 Project files in Diagram: the left panel switches between Library and Files; [ticket](tickets/KTLO-11.md); after KTLO-17 | M · 30 min · `visible UI`, `format` |
| [#43](https://github.com/jakubiszon26/apiforge/issues/43) | KTLO-18 The Problems section expands over the code with one click; [ticket](tickets/KTLO-18.md) | S · 15 min · `visible UI` |
| [#44](https://github.com/jakubiszon26/apiforge/issues/44) | KTLO-19 Run results can fill the window; [ticket](tickets/KTLO-19.md) | S · 20 min · `visible UI` |
| [#46](https://github.com/jakubiszon26/apiforge/issues/46) | KTLO-20 A sleeping dog keeps the empty board company in Diagram and Split; [ticket](tickets/KTLO-20.md) — last of the five wishes; animated (owner decision) | M · 30 min · `visible UI` |
| [#49](https://github.com/jakubiszon26/apiforge/issues/49) | KTLO-21 A reader still sees a live writer right after the writer is dropped (flaky on Linux CI); [ticket](tickets/KTLO-21.md) | S · 20 min; actual 393 lines (M), 1 round |
| [#51](https://github.com/jakubiszon26/apiforge/issues/51) | KTLO-22 Publishing from the header is sometimes not recorded yet when the test asserts it (flaky on Windows CI); [ticket](tickets/KTLO-22.md) — joined the sprint 2026-10-27 | S · 15 min |
| [#53](https://github.com/jakubiszon26/apiforge/issues/53) | KTLO-23a A component runs from its tab — runner, session and the Run button; [ticket](tickets/KTLO-23.md) | M · 45 min · `visible UI` |
| [#54](https://github.com/jakubiszon26/apiforge/issues/54) | KTLO-23b A component runs from its tab — the Run inputs dialog; [ticket](tickets/KTLO-23b.md); after KTLO-23a | S · 20 min · `visible UI` |

## Sprint: MCP (planned, after KTLO)

An MCP server: a model works on a project while the owner watches in the
window that is already open. Analysed 2026-10-27 ([MCP-00](tickets/MCP-00.md))
at the owner's request of 2026-10-26: a `mcp` subcommand of the `apiforge`
binary over stdio, so that Claude Code or another MCP client reads, checks,
edits, publishes and runs the scenarios of a folder project through API
Forge's own session — the same files and bytes the window writes. The owner
called it a sprint of its own and took the nine decisions of MCP-00 on
2026-10-27 ([#59](https://github.com/jakubiszon26/apiforge/issues/59)): 1–8
the recommended option and 9 = B (this sprint follows KTLO, LAY follows it).
The owner's audit of 2026-10-27 revised the analysis (MCP-00 revision 2):
the window stays open and observes the agent's committed changes live, one
writer per agent session, validation before the first write, versions on
every read and write, four defined ends of a run, one rule for secrets, a
written contract, and a per-project toggle for live requests. Decision 5
(the lock per write) is superseded; decisions 10–17 were taken 2026-10-27
(all A); 18 is open. [ADR
0014](decisions/0014-agent-sessions.md) records the ownership rules.

**Key acceptance criterion:** the agent works through MCP and the owner
watches its committed changes live in the already open application
(MCP-11).

Capacity: the owner review minutes of the MCP tickets below sum to **310**;
the interface findings carried from KTLO have their own line of **115**
(UX-00 to UX-05) and start after MCP-11 unless decision 18 says otherwise;
KTLO-31 adds 5. An addition made during the sprint is charged against the UX
line, and when it is spent an addition displaces a ticket, named at the
time. Carried from KTLO: the Windows run of the regression smoke list
(KTLO-07, #10), owed by the first MCP ticket that touches the window
(MCP-07), and the Stage II ignore `timing; run in release` (KTLO-31). Exit
criteria:

- [ ] In the demo shop folder, `claude mcp add apiforge -- apiforge mcp`
      lets a model list the project, read and check every source with line,
      column and version, print, simulate a scenario or a component and
      check a proposed text, with nothing written to the project (MCP-01,
      MCP-03).
- [ ] The language reference a model receives is a tested document whose
      examples parse, format unchanged and lower, served by the server
      (MCP-02, MCP-03).
- [ ] The model saves, registers and publishes through the server under one
      agent session: validated before the first byte, refused when stale,
      secrets never written (MCP-04, MCP-05).
- [ ] Live requests are off unless the person allows them — the window's
      toggle while a window is connected, `--allow-live` otherwise — and
      every end of a run has a defined result; no secret value leaves the
      server by any tool, resource, report, diagnostic, error or log
      (MCP-06, MCP-09).
- [ ] The window names who holds the project, hands it over on request from
      a consistent state, observes every commit within two seconds keeping
      the person's place, and takes the project back when the holder is
      gone — never from a living process — through mock-ups, acceptance
      tests and QA reports (MCP-07, MCP-08a, MCP-08b, MCP-08c, MCP-10).
- [ ] The whole workflow runs end to end with a real client while the owner
      watches, with the client's version and the steps recorded, and the
      Windows evidence or its absence stated item by item (MCP-11).

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#59](https://github.com/jakubiszon26/apiforge/issues/59) | MCP-00 Analysis and the owner's decisions; [ticket](tickets/MCP-00.md); revision 2 of 2026-10-27 | S (docs) · 30 min plus the discussion · decisions 1–17 taken 2026-10-27; 18 open |
| [#60](https://github.com/jakubiszon26/apiforge/issues/60) | MCP-01 `apiforge mcp` answers Claude Code: overview, read, check, print, with versions; [ticket](tickets/MCP-01.md) | M · 20 min · `default` |
| [#61](https://github.com/jakubiszon26/apiforge/issues/61) | MCP-02 A language reference for models, tested — the document; [ticket](tickets/MCP-02.md) | M (docs) · 30 min |
| [#66](https://github.com/jakubiszon26/apiforge/issues/66) | MCP-07 The lock names its holder; the window says who and takes the project back when it is free; [ticket](tickets/MCP-07.md); decision 6, mock-up M1 | S · 20 min · `visible UI`, `default` |
| [#62](https://github.com/jakubiszon26/apiforge/issues/62) | MCP-03 Run a simulation (scenario or component), check a proposal, serve the reference and resources; the executor returns data; cancellation and timeout; [ticket](tickets/MCP-03.md); after MCP-01, MCP-02 | M · 20 min |
| [#63](https://github.com/jakubiszon26/apiforge/issues/63) | MCP-04 Save and create from the server — the agent session, validation before the first write, versions, secrets, paths; [ticket](tickets/MCP-04.md); decisions 1, 2, 10; after MCP-03 | M · 30 min |
| [#90](https://github.com/jakubiszon26/apiforge/issues/90) | MCP-08a Observer mode: editing paused while another holds the project; the window follows every commit by reload within two seconds, keeping its navigation; [ticket](tickets/MCP-08a.md); decisions 10, 12, mock-up M1; after MCP-07 | M · 30 min · `visible UI`, `default`, `removal` |
| [#64](https://github.com/jakubiszon26/apiforge/issues/64) | MCP-05 Publish from the server: comment-loss consent, expected hash, request ids; [ticket](tickets/MCP-05.md); decision 3; after MCP-04 | S · 15 min |
| [#65](https://github.com/jakubiszon26/apiforge/issues/65) | MCP-06 Live requests: the four ends of a run, the journal, `--allow-live` and the permission slot; [ticket](tickets/MCP-06.md); decision 4; after MCP-04 | M · 25 min |
| [#91](https://github.com/jakubiszon26/apiforge/issues/91) | MCP-08b Hand-over on request: the control channel, a clean window hands over and observes, refusal with unsaved work, `saved` rescans, *Take back*, release; [ticket](tickets/MCP-08b.md); decisions 10, 11, mock-up M1; after MCP-08a, MCP-04 | M · 30 min · `visible UI`, `default` |
| [#92](https://github.com/jakubiszon26/apiforge/issues/92) | MCP-08c The hand-over prompt when the window has unsaved work; [ticket](tickets/MCP-08c.md); decision 13, mock-up M2; after MCP-08b | S · 15 min · `visible UI` |
| [#93](https://github.com/jakubiszon26/apiforge/issues/93) | MCP-09 The live toggle in the project settings; the server obeys it while a window is connected; [ticket](tickets/MCP-09.md); decisions 14–16, mock-up M3; after MCP-08b, MCP-06 | M · 25 min · `visible UI`, `default` |
| [#94](https://github.com/jakubiszon26/apiforge/issues/94) | MCP-10 The agent's run status in the header; [ticket](tickets/MCP-10.md); decision 17, mock-up M4; after MCP-08b | S · 15 min · `visible UI` |
| [#95](https://github.com/jakubiszon26/apiforge/issues/95) | MCP-11 The whole workflow end to end with a real client — the sprint's acceptance; [ticket](tickets/MCP-11.md); after every MCP ticket | S · 30 min |
| [#96](https://github.com/jakubiszon26/apiforge/issues/96) | KTLO-31 The `timing; run in release` ignore (`folder/tests.rs` frame budgets) names its ticket, or the test moves to the release-only verification step | S · 5 min |
| [#57](https://github.com/jakubiszon26/apiforge/issues/57) | KTLO-24 The bar with the open files hides itself when no file is open; [ticket](tickets/KTLO-24.md) — merged in #70 | S · 15 min · `visible UI`, `default` |
| [#79](https://github.com/jakubiszon26/apiforge/issues/79) | KTLO-26 A board edit keeps the model the printed source reads back, so the checkpoint it seals describes its source; [ticket](tickets/KTLO-26.md); decision A 2026-10-27 | S · 10 min · `format` |
| [#76](https://github.com/jakubiszon26/apiforge/issues/76) | KTLO-25 The demo Product component carries a type warning — merged in #82 | S · 5 min |
| [#84](https://github.com/jakubiszon26/apiforge/issues/84) | KTLO-27 An empty optional input in the Run inputs dialog means "absent"; [ticket](tickets/KTLO-27.md) — merged in #85 | S · 10 min · `visible UI` |
| [#86](https://github.com/jakubiszon26/apiforge/issues/86) | KTLO-28 Estimate by complexity and risk, not lines; one acceptable result is one pull request (retrospective action 1) — merged in #89 | S (docs) · 15 min · `process` |
| [#87](https://github.com/jakubiszon26/apiforge/issues/87) | KTLO-29 A hand-off path for GPT 6.1 SOL and Sonnet: `ready:*` labels, escalation, merge authority; [autonomy](process/autonomy.md) (action 2) — merged in #89 | S (docs) · 15 min · `process` |
| [#88](https://github.com/jakubiszon26/apiforge/issues/88) | KTLO-30 WIP experiment for MCP: four pull requests within 60 review minutes; actual minutes recorded (action 3) — merged in #89 | S (docs) · 10 min · `process` |

Order and parallelism: MCP-01, MCP-02 and MCP-07 depend on nothing and may
start together within the WIP cap; MCP-03 follows 01 and 02; MCP-04 follows
03; MCP-08a follows 07; MCP-05 and MCP-06 follow 04; MCP-08b follows 08a and
04; MCP-08c and MCP-10 follow 08b; MCP-09 follows 08b and 06; MCP-11 closes.

### Interface findings carried from KTLO — the UX line

Raised by the owner on 2026-10-27 after using the accepted KTLO changes
([retrospective](retrospectives/2026-10-27-ktlo.md#interface-findings-carried-into-the-mcp-sprint)),
ticketed at MCP planning ([UX-00](tickets/UX-00.md)): one shared mock-up,
five decisions, five tickets, 115 minutes in all, in the owner's priority.
They start after MCP-11 unless MCP-00 decision 18 says otherwise.

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#97](https://github.com/jakubiszon26/apiforge/issues/97) | UX-00 One mock-up and the owner's decisions for the five findings; [ticket](tickets/UX-00.md) | S · 20 min plus the discussion · `visible UI` · `decision-needed` |
| [#98](https://github.com/jakubiszon26/apiforge/issues/98) | UX-01 Several checks from one run | M · 30 min · `visible UI` |
| [#99](https://github.com/jakubiszon26/apiforge/issues/99) | UX-02 Run mode is a persistent setting in *Run options*; the Run button reads *Run simulation* (purple) or *Run live* | S · 20 min · `visible UI`, `default` |
| [#100](https://github.com/jakubiszon26/apiforge/issues/100) | UX-03 The red status line of the code editor goes | S · 10 min · `visible UI`, `removal` |
| [#101](https://github.com/jakubiszon26/apiforge/issues/101) | UX-04 *Running unsaved code* moves to the window header | S · 15 min · `visible UI` |
| [#102](https://github.com/jakubiszon26/apiforge/issues/102) | UX-05 Run results and Problems drag to the whole workspace | S · 20 min · `visible UI` |


## Later

Feature work resumes after KTLO exits. Candidates, unestimated:

- Panel widths carry over between projects opened in the same window: egui
  caches panel sizes under fixed IDs, so a restored per-project width loses
  to the cached one (project files, split code, diagnostics and, after
  KTLO-05c, the block library). One ticket for all panels.
- The block library's resize handle only grabs on the panel's side of its
  edge; the board takes the pointer on the other side (seen in KTLO-05c).
- The windows-2022 runner's display is 1024 × 768, so the CI captures
  requested at 1600 × 1000 come out at 1024 × 768 (KTLO-08 warning); set the
  runner's resolution or accept the smaller Windows evidence.
- The interaction-test `settle()` also waits for a debounced analysis whose
  pause has not passed (review finding on #19; not a cause of any failure).
- `resolving_one_documents_conflict_leaves_another_documents_unsaved_work_and_history`
  failed once on windows-2022 after #19 (run 40304041875) and passed on the
  same commit: a foreign write was not seen as a conflict. Timing-sensitive;
  needs a ticket if it repeats.
- `apiforge run --folder --component <name>`: the command line runs a saved
  component the way the window will after KTLO-23 (owner decision of
  2026-10-26 kept it out of that ticket).
- The *Run results* panel does not scroll to the selected step and cuts the
  fifth step off at its default height of 320 px (seen in the KTLO-23
  mock-up).
- Removal of the legacy JSON mode once the KTLO-06 parity list is closed.
- Roadmap M7 editor features ([roadmap](roadmap.md)).
- Windows release gates: interactive native QA and directory durability
  ([Stage II plan](stage-ii-code-workspace.md#delivery-and-acceptance)).

### Layout and wire routing readability

Analysed 2026-10-26 ([LAY-00](tickets/LAY-00.md)): a scenario opened from
code stacks in one column, no tidied wire is straight, wires are drawn on top
of each other, and ordering wires cross data wires by construction. Owner
decision of 2026-10-26: scheduled for the sprint after KTLO, in the order
below. LAY follows MCP instead, by the owner's decision of 2026-10-27
([#59](https://github.com/jakubiszon26/apiforge/issues/59),
[#32](https://github.com/jakubiszon26/apiforge/issues/32)):

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#32](https://github.com/jakubiszon26/apiforge/issues/32) | LAY-01 A quality measurement for layout and routing; [ticket](tickets/LAY-01.md) | S · 10 min |
| [#33](https://github.com/jakubiszon26/apiforge/issues/33) | LAY-02 A scenario opened from code is placed in layers; [ticket](tickets/LAY-02.md) | S · 15 min |
| [#34](https://github.com/jakubiszon26/apiforge/issues/34) | LAY-03 Wires are never drawn on top of each other; [ticket](tickets/LAY-03.md) | S · 15 min |
| [#35](https://github.com/jakubiszon26/apiforge/issues/35) | LAY-04 Ports decide rows: wires between neighbours run straight; [ticket](tickets/LAY-04.md) | M · 20 min |
| [#37](https://github.com/jakubiszon26/apiforge/issues/37) | LAY-06 The shape of a tidied scenario — owner decision with mock-ups; [ticket](tickets/LAY-06.md) | S · 30 min · `visible UI` · `decision-needed` |
| [#36](https://github.com/jakubiszon26/apiforge/issues/36) | LAY-05 Lanes between columns: a short wire does not cross a long one; [ticket](tickets/LAY-05.md) | M · 15 min · measured again after LAY-04 |
