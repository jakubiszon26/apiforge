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

## Sprint: MCP (2026-10-27 to 2026-11-02, closed)

Closed by the owner on 2026-11-02; the retrospective is owed. Tickets still
open at the close — MCP-11 (#95), MCP-19 (#191, PR #203), MCP-20 (#192),
REL-04 (#205, PR #206), KTLO-33 (#122, PR #204) and the issues of
2026-11-02 (#207–#219) — are placed by the [UX-1 plan](sprints/ux-1.md).

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
(the lock per write) is superseded; decisions 10–18 were taken 2026-10-27
(all A: MCP first, the UX line after MCP-11). [ADR
0014](decisions/0014-agent-sessions.md) records the ownership rules.

**Key acceptance criterion:** the agent works through MCP and the owner
watches its committed changes live in the already open application
(MCP-11).

Capacity: the owner review minutes of the MCP tickets below sum to **345**;
the interface findings carried from KTLO have their own line of **115**
(UX-00 to UX-05) and start after MCP-11 (decision 18 = A);
KTLO-31 adds 5. An addition made during the sprint is charged against the UX
line, and when it is spent an addition displaces a ticket, named at the
time; charged so far: REL-01 (30) and MCP-16 (30), 60 of the 115. Carried from KTLO: the Windows run of the regression smoke list
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
| [#59](https://github.com/jakubiszon26/apiforge/issues/59) | MCP-00 Analysis and the owner's decisions; [ticket](tickets/MCP-00.md); revision 2 of 2026-10-27 — merged in #67 and #103 | S (docs) · 30 min plus the discussion · decisions 1–18 taken 2026-10-27 |
| [#60](https://github.com/jakubiszon26/apiforge/issues/60) | MCP-01 `apiforge mcp` answers Claude Code: overview, read, check, print, with versions; [ticket](tickets/MCP-01.md) — merged in #105 | M · 20 min · `default` |
| [#61](https://github.com/jakubiszon26/apiforge/issues/61) | MCP-02 A language reference for models, tested — the document; [ticket](tickets/MCP-02.md) — merged in #104 | M (docs) · 30 min |
| [#66](https://github.com/jakubiszon26/apiforge/issues/66) | MCP-07 The lock names its holder; the window says who and takes the project back when it is free; [ticket](tickets/MCP-07.md); decision 6, mock-up M1 — merged in #106 | S · 20 min · `visible UI`, `default` |
| [#62](https://github.com/jakubiszon26/apiforge/issues/62) | MCP-03 Run a simulation (scenario or component), check a proposal, serve the reference and resources; the executor returns data; cancellation and timeout; [ticket](tickets/MCP-03.md); after MCP-01, MCP-02 | M · 20 min |
| [#63](https://github.com/jakubiszon26/apiforge/issues/63) | MCP-04 Save and create from the server — the agent session, validation before the first write, versions, secrets, paths; [ticket](tickets/MCP-04.md); decisions 1, 2, 10; after MCP-03 | M · 30 min |
| [#90](https://github.com/jakubiszon26/apiforge/issues/90) | MCP-08a Observer mode: editing paused while another holds the project; the window follows every commit by reload within two seconds, keeping its navigation; [ticket](tickets/MCP-08a.md); decisions 10, 12, mock-up M1; after MCP-07 | M · 30 min · `visible UI`, `default`, `removal` |
| [#64](https://github.com/jakubiszon26/apiforge/issues/64) | MCP-05 Publish from the server: comment-loss consent, expected hash, request ids; [ticket](tickets/MCP-05.md); decision 3; after MCP-04 | S · 15 min |
| [#65](https://github.com/jakubiszon26/apiforge/issues/65) | MCP-06 Live requests: the four ends of a run, the journal, `--allow-live` and the permission slot; [ticket](tickets/MCP-06.md); decision 4; after MCP-04 | M · 25 min |
| [#91](https://github.com/jakubiszon26/apiforge/issues/91) | MCP-08b Hand-over on request: the control channel, a clean window hands over and observes, refusal with unsaved work, `saved` rescans, *Take back*, release; [ticket](tickets/MCP-08b.md); decisions 10, 11, mock-up M1; after MCP-08a, MCP-04 | M · 30 min · `visible UI`, `default` |
| [#92](https://github.com/jakubiszon26/apiforge/issues/92) | MCP-08c The hand-over prompt when the window has unsaved work; [ticket](tickets/MCP-08c.md); decision 13, mock-up M2; after MCP-08b | S · 15 min · `visible UI` |
| [#93](https://github.com/jakubiszon26/apiforge/issues/93) | MCP-09 The live toggle in the project settings; the server obeys it while a window is connected; [ticket](tickets/MCP-09.md); decisions 14–16, mock-up M3; after MCP-08b, MCP-06 | M · 25 min · `visible UI`, `default` |
| [#94](https://github.com/jakubiszon26/apiforge/issues/94) | MCP-10 The agent's run status in the bottom bar; [ticket](tickets/MCP-10.md); decision 17, mock-up M4; after MCP-08b | S · 15 min · `visible UI` |
| [#183](https://github.com/jakubiszon26/apiforge/issues/183) | MCP-16 The observing window follows the agent's edits live; following can be unpinned in the bottom bar; [ticket](tickets/MCP-16.md); owner wish of 2026-11-01, decisions 1–7 taken 2026-11-01 (all A), mock-up f1–f4 approved 2026-11-01; after MCP-10; charged to the UX line | M · 30 min · `visible UI`, `default` |
| [#115](https://github.com/jakubiszon26/apiforge/issues/115) | REL-01 A GitHub release with an AppImage, a Windows ZIP and a Linux tar.gz; [ticket](tickets/REL-01.md); decisions 1–4 taken 2026-10-28; before MCP-11; charged to the UX line | M · 30 min · `visible UI` |
| [#123](https://github.com/jakubiszon26/apiforge/issues/123) | MCP-12 Discover a window holder and hand over across different process environments; [ticket](tickets/MCP-12.md); D1–D3 approved 2026-10-28; after MCP-08b, MCP-06 | M · 40 min · MCP output compatibility, `default` |
| [#95](https://github.com/jakubiszon26/apiforge/issues/95) | MCP-11 The whole workflow end to end with a real client — the sprint's acceptance; [ticket](tickets/MCP-11.md); after every MCP ticket | S · 30 min |
| [#96](https://github.com/jakubiszon26/apiforge/issues/96) | KTLO-31 The `timing; run in release` ignore (`folder/tests.rs` frame budgets) names its ticket, or the test moves to the release-only verification step | S · 5 min |
| [#181](https://github.com/jakubiszon26/apiforge/issues/181) | MCP-14 `run`'s structured output matches its own output schema; a test validates every tool's output against its `outputSchema`; found by the owner 2026-11-01; hotfix before other tickets, merge authorised on green CI | S · 10 min |
| [#182](https://github.com/jakubiszon26/apiforge/issues/182) | MCP-15 A live run from the command line asks the open window for the project over the control channel instead of refusing; [ticket](tickets/MCP-15.md); owner decision 2026-11-01, decisions 1–6 taken 2026-11-01 (all A), mock-up approved 2026-11-01; after MCP-12 | M · 30 min · `default`, `visible UI` |
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
04; MCP-08c and MCP-10 follow 08b; MCP-09 and MCP-12 follow 08b and 06;
MCP-15 follows MCP-12; MCP-16 follows MCP-10; MCP-11 closes, after MCP-12,
MCP-14 and MCP-15.

### Interface findings carried from KTLO — the UX line

Raised by the owner on 2026-10-27 after using the accepted KTLO changes
([retrospective](retrospectives/2026-10-27-ktlo.md#interface-findings-carried-into-the-mcp-sprint)),
ticketed at MCP planning ([UX-00](tickets/UX-00.md)): one shared mock-up,
five decisions, five tickets, 115 minutes in all, in the owner's priority.
They start after MCP-11 (MCP-00 decision 18 = A, 2026-10-27).

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#97](https://github.com/jakubiszon26/apiforge/issues/97) | UX-00 One mock-up and the owner's decisions for the five findings; [ticket](tickets/UX-00.md) | S · 20 min plus the discussion · `visible UI` · `decision-needed` |
| [#98](https://github.com/jakubiszon26/apiforge/issues/98) | UX-01 Several checks from one run | M · 30 min · `visible UI` |
| [#99](https://github.com/jakubiszon26/apiforge/issues/99) | UX-02 Run mode is a persistent setting in *Run options*; the Run button reads *Run simulation* (purple) or *Run live* | S · 20 min · `visible UI`, `default` |
| [#100](https://github.com/jakubiszon26/apiforge/issues/100) | UX-03 The red status line of the code editor goes | S · 10 min · `visible UI`, `removal` |
| [#101](https://github.com/jakubiszon26/apiforge/issues/101) | UX-04 *Running unsaved code* moves to the window header | S · 15 min · `visible UI` |
| [#102](https://github.com/jakubiszon26/apiforge/issues/102) | UX-05 Run results and Problems drag to the whole workspace | S · 20 min · `visible UI` |


## Sprint: UX-1 (in progress since 2026-11-02)

Usability and UX. The owner's words of 2026-11-02: the application's
usability must get more time now; many issues exist, some from tests in
real work, and those are solved first. The triage of every open interface
issue, the duplicates, the waves and the owner's decisions are in
[`docs/sprints/ux-1.md`](sprints/ux-1.md); the owner approved it the same
day with waves 1–4 (decision 1 = C) and two changes to the proposal:
*Save as component* is implemented (decision 17 = C, UX-30) and the
component mock-up COMP-00 is the sprint's first, made in parallel with wave
1, after the owner confirmed #174 from real work. LAY follows UX-1
(decision 2 = A); MCP-11 was closed without a report (decision 3 = C).

Capacity: **695** owner review minutes — wave 1 135, wave 2 245 (COMP-00,
decided 2026-11-02, and its two input tickets; UX-19 re-estimated M · 45 on
2026-11-05 with the REP-01 box), wave 3 180 (with the *Component settings*
button and the clipboard ticket), wave 4 135 (WS-00, RUN-00, MCP-20's
mock-up, UX-30, UX-06; DIAG-00 moved to the LAY sprint on 2026-11-05). The tickets the remaining
mock-ups produce are estimated on the mock-ups and added here then. The
work-in-progress limit of four pull requests within 60 minutes is the order
of work; the owner allows UX-1 to exceed it except for `L` tickets, which
are never scheduled. In flight from MCP while open: REL-04 (#205), KTLO-33
(#122); MCP-19 (#191) was merged on 2026-11-02. Exit criteria:

- [ ] The five decided findings of UX-00 are accepted (UX-01..05,
      #98–#102).
- [ ] The critical real-work problems are accepted: the live toggle's
      *Cancel* (#214), the first window fits the screen (#215), the three
      misleading MCP fields (#208).
- [ ] Every real-work editor and notice defect of 2026-11-02 is accepted
      (#216, #217, #218, #193).
- [ ] The verified automated defects without a design question are
      accepted (wave 2) and the wave-3 tickets as decided.
- [ ] The shared mock-ups COMP-00 (decided 2026-11-02), WS-00 (decided
      2026-11-05), RUN-00 (DIAG-00 moved to the LAY sprint on 2026-11-05) and
      MCP-20's are decided by the owner and their tickets estimated;
      *Save as component* (UX-30) is accepted.
- [ ] The UX-1 retrospective is written with the WIP measures KTLO-30 asked
      for.

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#98](https://github.com/jakubiszon26/apiforge/issues/98) | UX-01 Several checks from one run; decided 2026-11-02 on UX-00's mock-up | M · 30 min · `visible UI` |
| [#99](https://github.com/jakubiszon26/apiforge/issues/99) | UX-02 Run mode is a setting; *Run simulation* / *Run live*; *Run options* is a chevron; decided 2026-11-02 | S · 20 min · `visible UI`, `default` |
| [#100](https://github.com/jakubiszon26/apiforge/issues/100) | UX-03 The focus line turns grey; error marks after a pause; decided 2026-11-02 | S · 10 min · `visible UI`, `removal` |
| [#101](https://github.com/jakubiszon26/apiforge/issues/101) | UX-04 *Running unsaved code* in the header while the run runs (absorbs #152); decided 2026-11-02; after UX-02 | S · 15 min · `visible UI` |
| [#102](https://github.com/jakubiszon26/apiforge/issues/102) | UX-05 Run results and Problems drag to the whole workspace; decided 2026-11-02; after UX-01 | S · 20 min · `visible UI` |
| [#214](https://github.com/jakubiszon26/apiforge/issues/214) | UX-08 *Cancel* in Project settings leaves no live permission behind; the sentence states the reach (MCP-26, critical); decision 4 | S · 15 min · `visible UI`, `default` |
| [#215](https://github.com/jakubiszon26/apiforge/issues/215) | UX-09 The first window fits the screen's work area (KTLO-38, critical); decision 5 | S · 15 min · `default` |
| [#208](https://github.com/jakubiszon26/apiforge/issues/208) | UX-10 Three MCP result fields say what happened (MCP-22, critical); non-UI | S · 10 min |
| [#239](https://github.com/jakubiszon26/apiforge/issues/239) | COMP-00 Component authoring in Diagram — one mock-up: inputs on the board, the Inspector, *Save as component*, *Publish* (#174 confirmed from real work, #161 (2), #172 (1, 2), #173, #170 (1), #165); decided 2026-11-02 on `mock/comp-00` at `5ea3b9c` (Q1 = C strip, Q2 = A, Q3 as captured + live edits, Q4 header button, Q5 both outcomes, Q6 = B, Q7 own ticket); the ticket file lands with UX-31 | S · 30 min plus the discussion · `visible UI` |
| [#248](https://github.com/jakubiszon26/apiforge/issues/248) | UX-31 A component's INPUTS strip: pills, a ring on the reading steps, *+ Add input* (#174 real work; COMP-00 Q1 = C); after COMP-00 | M · 30 min · `visible UI` |
| [#249](https://github.com/jakubiszon26/apiforge/issues/249) | UX-32 Inputs added and edited in the Inspector, live, with inline errors (#174, #172 (1, 2); COMP-00 Q2 = A, Q3); after UX-31 | M · 30 min · `visible UI` |
| [#221](https://github.com/jakubiszon26/apiforge/issues/221) | UX-11 No completion in a comment; a click after a line's end focuses the editor (#216 KTLO-39, #217 KTLO-40); after UX-03 | S · 15 min |
| [#222](https://github.com/jakubiszon26/apiforge/issues/222) | UX-12 Non-error notices clear; the error banner is per document (#218 KTLO-41, #175 (4)); decision 6 = A; after UX-04 | S · 10 min · `default` |
| [#193](https://github.com/jakubiszon26/apiforge/issues/193) | UX-13 A tab keeps its scroll across switches; decision 7; after UX-11 | S · 15 min · `default` |
| [#202](https://github.com/jakubiszon26/apiforge/issues/202) | UX-14 *Extract component* proposes *Required* unticked for an argument with a default (KTLO-36); decision 8 | S · 10 min · `visible UI`, `default` |
| [#223](https://github.com/jakubiszon26/apiforge/issues/223) | UX-15 The unsaved dot survives a truncated tab name (with #176 (1)) | S · 10 min · `visible UI` |
| [#224](https://github.com/jakubiszon26/apiforge/issues/224) | UX-16 Cleanup cards resolve references to Cleanup steps | S · 10 min |
| [#225](https://github.com/jakubiszon26/apiforge/issues/225) | UX-17 *Tidy up* with one card or a chip selected tidies the board (with #168 (4)); decision 9 | S · 10 min · `default` |
| [#226](https://github.com/jakubiszon26/apiforge/issues/226) | UX-18 The Inspector opening does not re-fit the board; the selected or new card stays visible (with #156 (1)) | M · 20 min · `default` |
| [#227](https://github.com/jakubiszon26/apiforge/issues/227) | UX-19 Run results navigation: scroll to the selected row, selection on a failed run, collapsible component rows, breadcrumb, the REP-01 box (#135); decided 2026-11-05 ([ux-1.md 6.2](sprints/ux-1.md#62-ux-19-and-ws-00-decisions-2026-11-05)); after UX-05 | M · 45 min · `visible UI`, `default` |
| [#228](https://github.com/jakubiszon26/apiforge/issues/228) | UX-20 Gutter room for the dot, amber for warnings; a plain message for undo on invalid source (#153 (1)); the popup clears the Problems header (#170 (5)); after UX-13 | S · 10 min · `visible UI` |
| [#229](https://github.com/jakubiszon26/apiforge/issues/229) | UX-21 *Export* in *Add step* inside components; *Start* not offered there; *No matching steps*; decision 12 | S · 15 min · `visible UI`, `removal` |
| [#230](https://github.com/jakubiszon26/apiforge/issues/230) | UX-22 A component is not offered inside itself; *New component* lands in `components/`; decision 13; after UX-15 | S · 15 min · `default` |
| [#231](https://github.com/jakubiszon26/apiforge/issues/231) | UX-23 The manifest is protected in the Explorer; a text file in Diagram offers *Show code*; decisions 14, 15; after UX-22 | S · 10 min · `visible UI`, `removal` |
| [#232](https://github.com/jakubiszon26/apiforge/issues/232) | UX-24 *Update to revision N* succeeds when ports change; an update badge; defaults shown; mock-up for the badge; after UX-18 | M · 30 min · `visible UI` |
| [#233](https://github.com/jakubiszon26/apiforge/issues/233) | UX-25 *Publish* disabled with *r1 · no changes* beside it; no banner flash; the stale-results rule; decision 16 = A, COMP-00 Q6 = B; after UX-04 | S · 15 min · `visible UI`, `default` |
| [#234](https://github.com/jakubiszon26/apiforge/issues/234) | UX-26 Wording, plurals, hints, *Skipped* in the summary (with #156 (2)); after UX-19 | S · 10 min · `visible UI` |
| [#235](https://github.com/jakubiszon26/apiforge/issues/235) | UX-27 Distinct accessible labels; after UX-26 | S · 10 min |
| [#236](https://github.com/jakubiszon26/apiforge/issues/236) | UX-28 *Curves* removed until it draws curves; a saved preference falls back to *Steps* (#170); decision 18 = A; after UX-17 | S · 10 min · `removal`, `visible UI` |
| [#147](https://github.com/jakubiszon26/apiforge/issues/147) | UX-07 A wider Inspector; one capture per size; after UX-24 | S · 15 min · `visible UI` |
| [#237](https://github.com/jakubiszon26/apiforge/issues/237) | UX-29 Card controls clipped to the board (#172 (3)); after UX-18 | S · 10 min |
| [#250](https://github.com/jakubiszon26/apiforge/issues/250) | UX-33 A *Component settings* button in the board header opens the component's settings (#161 (2); COMP-00 Q4, button not rendered, judged on QA captures); after UX-32 | S · 20 min · `visible UI` |
| [#251](https://github.com/jakubiszon26/apiforge/issues/251) | UX-34 Copy, paste and duplicate across graphs and tabs; duplicates land clear with their own label (#173; COMP-00 Q7); A/B on its captures; after COMP-00 | M · 20 min · `default` · `decision-needed` |

| [#240](https://github.com/jakubiszon26/apiforge/issues/240) | WS-00 The workspace — one mock-up: a shared left sidebar (UX-06 #146), what folds at 960 × 540 (#157), Split's four columns (#161 (4)); decided 2026-11-05, [ticket](tickets/WS-00.md) | S · 30 min plus the discussion · `visible UI`, `default` |
| [#241](https://github.com/jakubiszon26/apiforge/issues/241) | DIAG-00 Card readability — closed 2026-11-05, folded into the LAY sprint (LAY-09 #293, LAY-11 #295; the owner's Q8 = A); DIAG-01 #144 deferred to its own analysis after LAY | — |
| [#242](https://github.com/jakubiszon26/apiforge/issues/242) | RUN-00 Run inputs, settings and the JSON response view — one mock-up (#161 (3, 5), REP-04 #148); after UX-01, UX-02 | S · 30 min plus the discussion · `visible UI`, `default` · `decision-needed` |
| [#192](https://github.com/jakubiszon26/apiforge/issues/192) | MCP-20 A command's live run shows its status in the observing window's bar; mock-up first | S · 15 min · `visible UI` |
| [#238](https://github.com/jakubiszon26/apiforge/issues/238) | UX-30 *Save as component* in a folder project (#170 (1)); decision 17 = C; COMP-00 Q5: both outcomes offered, publish-and-call the default; after COMP-00 | M · 30 min · `visible UI` |
| [#146](https://github.com/jakubiszon26/apiforge/issues/146) | UX-06 Shared left sidebar; the Explorer available in Split; estimated on WS-00 | M · 30 min · `visible UI`, `default` |

Wave 1 is #98–#102, #214, #215, #208; wave 2 runs from COMP-00 to UX-20;
wave 3 holds UX-21..UX-29, UX-07, UX-33 and UX-34; wave 4 holds the three remaining mock-ups,
MCP-20, UX-30 and UX-06. The tickets a mock-up produces join this table
with the estimate taken on the mock-up.

Out of the sprint, with the reasons in the plan: LAY-01..06 and the routing
findings (#167, #168 (1–3), #170 (4)); the functional gaps #125–#143 and
#124; the MCP and language issues that are not critical (#207, #209, #210,
#211, #212, #213); #219 (tooling, a Sonnet fix); #155 (parser); #158 (not
reproduced); #116; #201 (docs, merge on green CI).

## Sprint: LAY (in progress since 2026-11-05)

Diagram comprehension. The owner's brief of 2026-11-03: a person opening a
large scenario must understand, without effort or manual tidying, what runs
after what, which checks verify which results and where data comes from.
The audit, the UX model, four rounds of mock-ups with measurements, three
second opinions and the owner's decisions Q1–Q16 of 2026-11-05 are in
[`docs/sprints/lay.md`](sprints/lay.md). One decision widened the proposal:
the three deferred items that change what the board says (LAY-17, LAY-18,
LAY-19) are in (Q12 = C). DIAG-00 (#241) folded into LAY-09 and LAY-11
(Q8 = A). Runs after UX-1's wave 3; UX-1's wave 4 continues beside it.

Capacity: **365** owner review minutes over 19 tickets — must 255, should 50,
added 60. The order of work is the work-in-progress limit of four pull
requests within 60 minutes; waves ([plan, section 7](sprints/lay.md#7-dependencies-and-order)):
0 LAY-01, LAY-08 (no decision, may start now) · 1 LAY-16 → 09 → 07 → 19 →
10 → 14, 15 → 18 → 11 · 2 LAY-02, 06, 17, 13 · 3 LAY-04, 03, 05 · 4 LAY-12.
Exit criteria:

- [ ] The three reference scenarios (14, 30, 50 steps) exist and the metric
      command prints their table (LAY-01, #32).
- [ ] A 30-step scenario opened from code lays itself out in shape D and
      fits a 1600 × 1000 window at 50 % or more (LAY-02, LAY-06, LAY-16).
- [ ] Order, checks and data are read from the board: badges equal the Run
      results rows, checks collapse into their subject's footer, focus and
      tracing name the sources (LAY-07, LAY-11, LAY-10, LAY-14, LAY-15).
- [ ] The three items the owner added (Q12 = C) are in: the row break
      respects the wires, a request's card says which fields are read and
      a failure says what it cost (LAY-17 #301, LAY-18 #302, LAY-19 #303).
- [ ] The UX task sheet is timed before and after; the demo plan runs end to
      end at both window sizes (LAY-12, #296).

| Issue | Ticket | Estimate · risk | Wave |
| --- | --- | --- | --- |
| [#32](https://github.com/jakubiszon26/apiforge/issues/32) | LAY-01 Metrics and the reference scenarios; [ticket](tickets/LAY-01.md) | S · 10 | 0 · `ready:opus` |
| [#292](https://github.com/jakubiszon26/apiforge/issues/292) | LAY-08 Check subjects in core, the Inspector's list; [ticket](tickets/LAY-08.md) | S · 20 · `visible UI` | 0 · `ready:opus` |
| [#300](https://github.com/jakubiszon26/apiforge/issues/300) | LAY-16 Compact request geometry; [ticket](tickets/LAY-16.md) | M · 25 · `visible UI`, `default` | 1 · `ready:opus` |
| [#293](https://github.com/jakubiszon26/apiforge/issues/293) | LAY-09 Semantic zoom with minimum sizes; [ticket](tickets/LAY-09.md) | S–M · 20 · `visible UI` | 1 |
| [#291](https://github.com/jakubiszon26/apiforge/issues/291) | LAY-07 Sequence badges and the running step; [ticket](tickets/LAY-07.md) | S · 15 · `visible UI` | 1 |
| [#303](https://github.com/jakubiszon26/apiforge/issues/303) | LAY-19 Failure consequences by cause; [ticket](tickets/LAY-19.md) | M · 20 · `visible UI` | 1 |
| [#294](https://github.com/jakubiszon26/apiforge/issues/294) | LAY-10 Focus on selection, the relation model; [ticket](tickets/LAY-10.md) | S–M · 20 · `visible UI`, `default` | 1 |
| [#298](https://github.com/jakubiszon26/apiforge/issues/298) | LAY-14 Wire and input tracing; [ticket](tickets/LAY-14.md) | S–M · 20 · `visible UI` | 1 |
| [#299](https://github.com/jakubiszon26/apiforge/issues/299) | LAY-15 Off-screen relations, chips and Back; [ticket](tickets/LAY-15.md) | M · 20 · `visible UI` | 1 |
| [#302](https://github.com/jakubiszon26/apiforge/issues/302) | LAY-18 Compact shows the data in use; [ticket](tickets/LAY-18.md) | M · 20 · `visible UI` | 1 |
| [#295](https://github.com/jakubiszon26/apiforge/issues/295) | LAY-11 Collapse checks; [ticket](tickets/LAY-11.md) | M · 30 · `visible UI`, `default` | 1 |
| [#33](https://github.com/jakubiszon26/apiforge/issues/33) | LAY-02 Auto-layout on open, chips; [ticket](tickets/LAY-02.md) | S–M · 15 · `default` | 2 |
| [#37](https://github.com/jakubiszon26/apiforge/issues/37) | LAY-06 The shape: D hybrid; [ticket](tickets/LAY-06.md) | M · 30 · `default` | 2 |
| [#301](https://github.com/jakubiszon26/apiforge/issues/301) | LAY-17 Wrapping by dependencies; [ticket](tickets/LAY-17.md) | M · 20 · `default` | 2 |
| [#297](https://github.com/jakubiszon26/apiforge/issues/297) | LAY-13 Branch bands; [ticket](tickets/LAY-13.md) | M · 20 · `visible UI` | 2 (should) |
| [#35](https://github.com/jakubiszon26/apiforge/issues/35) | LAY-04 Ports decide rows; [ticket](tickets/LAY-04.md) | M · 20 | 3 |
| [#34](https://github.com/jakubiszon26/apiforge/issues/34) | LAY-03 Nudge; [ticket](tickets/LAY-03.md) | S · 15 | 3 (should) |
| [#36](https://github.com/jakubiszon26/apiforge/issues/36) | LAY-05 Lanes, measured first; [ticket](tickets/LAY-05.md) | M · 15 | 3 (should) |
| [#296](https://github.com/jakubiszon26/apiforge/issues/296) | LAY-12 Acceptance: the demo and the timed task sheet; [ticket](tickets/LAY-12.md) | S · 10 | 4 |

Deferred to LAY-2 with the reasons in the plan: DIAG-01 descriptions
(`format`), a component summary on the parent board, MCP-13, arrowheads
(#167 (3)), wire-length metrics beyond LAY-01. Mock branches `mock/lay-00-*`
stay on origin because the issues' images point at their commits.

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

The owner's brief of 2026-11-03 widened the line from geometry to
comprehension — what runs after what, what checks what, where data comes
from — and asked for a sprint after UX-1. Its audit, UX model, decisions and
proposed tickets are in [`docs/sprints/lay.md`](sprints/lay.md), decided
by the owner's quiz of 2026-11-05 and now the
[LAY sprint](#sprint-lay-in-progress-since-2026-11-05) above. The rows below
are the 2026-10-26 tickets it re-audited, kept as the record.

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
