# Backlog

Tickets are [GitHub issues](https://github.com/jakubiszon26/apiforge/issues),
one per ticket, titled `<ID>: <goal>` and labelled as the
[development process](process/README.md#labels) says. Their state — open,
closed, `decision-needed`, `awaiting-acceptance` — lives there and nowhere
else. This file names the current sprint, its exit criteria and the issues
in it, and keeps the record of finished sprints. Where the work stood at
the end of the last working day, and what comes next, is in the
[handoff](handoff.md).

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
| [#251](https://github.com/jakubiszon26/apiforge/issues/251) | UX-34 Copy, paste and duplicate across graphs and tabs; duplicates land clear with their own label (#173; COMP-00 Q7); A/B on its captures; after COMP-00 | M · 20 min · `default` · merged in #268 (2026-11-05) |

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

## Sprint: LAY (2026-11-05 to 2026-11-19, closed)

**Closed on 2026-11-19** by the owner ([#296](https://github.com/jakubiszon26/apiforge/issues/296), decision 13 = A). The
owner ran the build that holds the minimal set (`1d11f07`) on a scenario
of their own — sixteen independent negative requests to one endpoint, each
with one to four checks — instead of the room-booking task. No line was
lost, no data was taken for order and no connection needed several clicks,
so the hard condition of [The close](sprints/lay.md#131-the-criterion)
holds; what the run found about the board went to LAY-2, and what it found
about reading results and editing a request opened the
[REQ sprint](#next-sprint-req-not-started). The findings, their causes
and the fifteen decisions are in [sprints/req.md](sprints/req.md). The
[retrospective](retrospectives/2026-11-20-lay.md) has the facts, the exit
criteria below with their evidence, and three actions (KTLO-44 [#475](https://github.com/jakubiszon26/apiforge/issues/475),
KTLO-45 [#479](https://github.com/jakubiszon26/apiforge/issues/479), KTLO-46 [#480](https://github.com/jakubiszon26/apiforge/issues/480)).

LAY-2 (`sprint:lay-2`, not scheduled) holds what was deferred on
2026-11-18 — LAY-13 [#297](https://github.com/jakubiszon26/apiforge/issues/297), LAY-17 [#301](https://github.com/jakubiszon26/apiforge/issues/301), LAY-35 [#410](https://github.com/jakubiszon26/apiforge/issues/410), LAY-36
[#419](https://github.com/jakubiszon26/apiforge/issues/419), LAY-38 [#427](https://github.com/jakubiszon26/apiforge/issues/427), [#445](https://github.com/jakubiszon26/apiforge/issues/445) — and what this run added: LAY-43
[#466](https://github.com/jakubiszon26/apiforge/issues/466) a check that reads two steps is drawn once, LAY-44 [#467](https://github.com/jakubiszon26/apiforge/issues/467)
folded checks take no room, LAY-45 [#468](https://github.com/jakubiszon26/apiforge/issues/468) the ordering lines of a chain,
LAY-46 [#469](https://github.com/jakubiszon26/apiforge/issues/469) the checks line at the Overview zoom after a run. W4 to
W9 of the plan's 13.2 still have no issues of their own. Its geometry is
measured again after LANG-13 ([13.6](sprints/lay.md#136-to-measure-again-after-lang-13)).


Diagram comprehension. The owner's brief of 2026-11-03: a person opening a
large scenario must understand, without effort or manual tidying, what runs
after what, which checks verify which results and where data comes from.
The audit, the UX model, four rounds of mock-ups with measurements, three
second opinions and the owner's decisions Q1–Q16 of 2026-11-05 are in
[`docs/sprints/lay.md`](sprints/lay.md). One decision widened the proposal:
the three deferred items that change what the board says (LAY-17, LAY-18,
LAY-19) are in (Q12 = C). DIAG-00 (#241) folded into LAY-09 and LAY-11
(Q8 = A). Runs after UX-1's wave 3; UX-1's wave 4 continues beside it.

What follows the checkpoint — the owner's order of part B, the tickets added
ahead of it and in it (DIAG-07, LAY-29 to LAY-34, 120 minutes; LAY-04 closed) and where the notes of
2026-11-16 go — is in [After the checkpoint](sprints/lay.md#12-after-the-checkpoint-2026-11-16).

**Shortened on 2026-11-18** by the owner (#296): the sprint closes on a
tester's task on the room-booking scenario, not on the demo plan and the
task sheet. The criterion, the minimal set that remains (#433, LAY-30
re-scoped, LAY-41 #437, LAY-15, LAY-42 #439, LAY-39 #435, LAY-40 #436,
then LAY-12), what moved to LAY-2 (LAY-13 #297, LAY-17 #301, LAY-35 #410,
LAY-36 #419, LAY-38 #427, `sprint:lay-2`) and what is measured again after
LANG-13 are in [The close](sprints/lay.md#13-the-close-2026-11-18). No new
board-geometry work until LAY-2.

Capacity: **385** owner review minutes over 21 tickets. The order of work
was revised by the [mid-sprint review of 2026-11-10](sprints/lay.md#11-mid-sprint-review-2026-11-10),
which replaces the waves: **part A**, the coherent board — LAY-16 → LAY-21 →
LAY-11 → LAY-09 → LAY-07 on the card track, LAY-02 → LAY-06 → LAY-04 →
LAY-03 on the layout track and LAY-22 on the engine track, in parallel; a **checkpoint** where
the owner uses the real board and confirms part B; **part B** — LAY-10 →
LAY-14, LAY-15 → LAY-19 → LAY-18 → LAY-17, LAY-13 → LAY-05 → LAY-12. The *Wave* column below is the plan of 2026-11-05, kept as the
record.
Exit criteria (the last two were replaced on 2026-11-18 by the criterion
of [The close](sprints/lay.md#131-the-criterion): the owner does the task
of [LAY-12](tickets/LAY-12.md) on the build that holds the minimal set,
and the wires pass its hard condition):

- [x] The three reference scenarios (14, 30, 50 steps) exist and the metric
      command prints their table (LAY-01, #32, merged in #305; the command
      run on `main` `eb2dc01`, 2026-11-20).
- [x] A 30-step scenario opened from code lays itself out in shape D and
      fits a 1600 × 1000 window at 50 % or more (LAY-02 #319, LAY-06 #330,
      LAY-16 #310). Read with checks collapsed (the owner, 2026-11-16):
      51.5 % for the board by the metric command, 49 % in the window.
- [x] Order, checks and data are read from the board: badges equal the Run
      results rows, checks collapse into their subject's footer, focus and
      tracing name the sources (LAY-07 #338, LAY-11 #327, LAY-10 #401,
      LAY-14 #420, LAY-15 #442; the hard condition of the owner's run,
      #296). One defect stays: LAY-43 #466.
- [ ] The three items the owner added (Q12 = C) are in: the row break
      respects the wires, a request's card says which fields are read and
      a failure says what it cost. **Two of three**: LAY-18 (#428) and
      LAY-19 (#422) are merged; LAY-17 #301 moved to LAY-2.
- [ ] The UX task sheet is timed before and after; the demo plan runs end to
      end at both window sizes (LAY-12, #296). **Not met; replaced twice**:
      by the tester's task of 2026-11-18, which only an agent ran, and by
      the owner's own run of 2026-11-19, taken as the acceptance (decision
      13 = A). The hard condition on the wires holds; the task, the timing
      and the demo plan were not done by a person.

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
| [#299](https://github.com/jakubiszon26/apiforge/issues/299) | LAY-15 Off-screen relations: edge chips, the jump and Back; [ticket](tickets/LAY-15.md); amended 2026-11-18 (Q4 = A) | M · 20 · `visible UI` | the close |
| [#302](https://github.com/jakubiszon26/apiforge/issues/302) | LAY-18 Compact shows the data in use; [ticket](tickets/LAY-18.md) | M · 20 · `visible UI` | 1 |
| [#295](https://github.com/jakubiszon26/apiforge/issues/295) | LAY-11 Collapse checks; [ticket](tickets/LAY-11.md) | M · 30 · `visible UI`, `default` | 1 |
| [#33](https://github.com/jakubiszon26/apiforge/issues/33) | LAY-02 Auto-layout on open, chips; [ticket](tickets/LAY-02.md) | S–M · 15 · `default` | 2 |
| [#37](https://github.com/jakubiszon26/apiforge/issues/37) | LAY-06 The shape: D hybrid; [ticket](tickets/LAY-06.md) | M · 20 · `default` | 2 |
| [#301](https://github.com/jakubiszon26/apiforge/issues/301) | LAY-17 Wrapping by dependencies; [ticket](tickets/LAY-17.md); moved to LAY-2 on 2026-11-18 | M · 20 · `default` | 2 |
| [#297](https://github.com/jakubiszon26/apiforge/issues/297) | LAY-13 Branch bands; [ticket](tickets/LAY-13.md); moved to LAY-2 on 2026-11-18, after LANG-13 | M · 20 · `visible UI` | 2 (should) |
| [#35](https://github.com/jakubiszon26/apiforge/issues/35) | LAY-04 Ports decide rows; [ticket](tickets/LAY-04.md) | M · 20 | 3 |
| [#34](https://github.com/jakubiszon26/apiforge/issues/34) | LAY-03 Nudge; [ticket](tickets/LAY-03.md) | S · 15 | 3 (should) |
| [#36](https://github.com/jakubiszon26/apiforge/issues/36) | LAY-05 Lanes, measured first; [ticket](tickets/LAY-05.md) | M · 15 | 3 (should) |
| [#296](https://github.com/jakubiszon26/apiforge/issues/296) | LAY-12 Acceptance: the owner's task on the room-booking scenario; [ticket](tickets/LAY-12.md); rewritten 2026-11-18 | S · 10 | the close, last |
| [#311](https://github.com/jakubiszon26/apiforge/issues/311) | LAY-21 Every other card is compact too; [ticket](tickets/LAY-21.md); from LAY-06, which becomes M · 20 | S–M · 20 · `visible UI`, `default` | part A |
| [#312](https://github.com/jakubiszon26/apiforge/issues/312) | LAY-22 Runner and core groundwork: the started step, the not-run cause, relations; [ticket](tickets/LAY-22.md) | S–M · 10 | part A |
| [#333](https://github.com/jakubiszon26/apiforge/issues/333) | LAY-23 A value many steps read: thin the board's lines; [ticket](tickets/LAY-23.md); decided 2026-11-12, stage 1 in implementation | M · 30 · `visible UI`, `default`, `removal` | part A, before the checkpoint |
| [#335](https://github.com/jakubiszon26/apiforge/issues/335) | LAY-25 A value used far away is saved in a variable, proposed by the board; [ticket](tickets/LAY-25.md); decided 2026-11-15 | M · 35 · `visible UI`, `default` | after LAY-23 stage 1 |
| [#341](https://github.com/jakubiszon26/apiforge/issues/341) | LAY-26 Code and the command line warn where a far value is read directly; [ticket](tickets/LAY-26.md); decided 2026-11-15, split from LAY-25; checked against the merged LAY-25 | S–M · 15 · `visible UI` | before LAY-24 |
| [#334](https://github.com/jakubiszon26/apiforge/issues/334) | LAY-24 A mark on the card replaces the `@after` rail into the next step; [ticket](tickets/LAY-24.md); decided 2026-11-15 (the arrow at the badge) | S–M · 20 · `visible UI`, `default`, `removal` | before the checkpoint, after LAY-25 and LAY-26 |
| [#344](https://github.com/jakubiszon26/apiforge/issues/344) | LAY-27 A collapsed check line carries the checks' numbers (`7 · 1 check`); [ticket](tickets/LAY-27.md); decided 2026-11-15 | S · 15 · `visible UI` | after LAY-25 |
| [#348](https://github.com/jakubiszon26/apiforge/issues/348), [#349](https://github.com/jakubiszon26/apiforge/issues/349) | LAY-28 Lines in one corridor are ordered so they cross less, and the inputs of a *Data condition* are 20 px apart; [ticket](tickets/LAY-28.md); #349 decided 2026-11-16 (A) | M · 25 · `visible UI` | before the checkpoint, with the owner's decision of 2026-11-16 |
| [#383](https://github.com/jakubiszon26/apiforge/issues/383) | LAY-30 A step with one check writes it on its folded line; the checks switch on the bar and on each step; re-scoped 2026-11-18 (Q1 = A, Q2 = A), mock-up with the owner | M · 25 · `visible UI` | the close |
| [#435](https://github.com/jakubiszon26/apiforge/issues/435) | LAY-39 A chip keeps the index of the element it reads; decided 2026-11-18 (Q7 = A) | S · 10 · `visible UI` | the close |
| [#436](https://github.com/jakubiszon26/apiforge/issues/436) | LAY-40 A scenario opens in Diagram in a project never opened here; decided 2026-11-18 (Q8 = A) | S · 5 · `default` | the close |
| [#437](https://github.com/jakubiszon26/apiforge/issues/437) | LAY-41 At the Overview zoom a card says what the step is; decided in principle 2026-11-18 (Q3 = A), mock-up with the owner | S · 15 · `visible UI` | the close, after LAY-30 |
| [#439](https://github.com/jakubiszon26/apiforge/issues/439) | LAY-42 A field's source is changed from the field and applied only on Apply; decided in principle 2026-11-18 (Q5 = A, Q6 = A), mock-up with the owner | M · 25 · `visible UI` | the close |
| [#433](https://github.com/jakubiszon26/apiforge/issues/433) | A check added from Run results is written in its subject's braces | S · 10 | the close |

Nine UX-1 tickets joined the sprint on 2026-11-05 at the owner's request,
keeping `sprint:ux` beside `sprint:lay`. Their pull requests were accepted in
the quizzes of 2026-11-03, on condition of merging `main` and a green CI, and
eight had fallen into conflict with `main`. They merged one at a time before
LAY-16, because they change the same board files; the last went in on
2026-11-07. Accepted already, they add nothing to the sprint's review minutes.

| Issue | Ticket | Pull request | Estimate |
| --- | --- | --- | --- |
| [#222](https://github.com/jakubiszon26/apiforge/issues/222) | UX-12 Notices clear | #267 | S · 10 |
| [#251](https://github.com/jakubiszon26/apiforge/issues/251) | UX-34 Copy, paste and duplicate | #268 | M · 20 |
| [#233](https://github.com/jakubiszon26/apiforge/issues/233) | UX-25 Publish with nothing to publish | #278 | S · 15 |
| [#226](https://github.com/jakubiszon26/apiforge/issues/226) | UX-18 The Inspector does not re-fit the board | #283 | M · 20 |
| [#229](https://github.com/jakubiszon26/apiforge/issues/229) | UX-21 Add step inside a component | #280 | S · 15 |
| [#224](https://github.com/jakubiszon26/apiforge/issues/224) | UX-16 Cleanup references | #277 | S · 10 |
| [#236](https://github.com/jakubiszon26/apiforge/issues/236) | UX-28 Curves removed | #282 | S · 10 |
| [#228](https://github.com/jakubiszon26/apiforge/issues/228) | UX-20 The gutter and diagnostics | #281 | S · 10 |
| [#223](https://github.com/jakubiszon26/apiforge/issues/223) | UX-15 The unsaved dot | #273 | S · 10 |

[#308](https://github.com/jakubiszon26/apiforge/issues/308) LAY-20, found in
LAY-08, joined the sprint on 2026-11-05: the *Checks* list covers a component
call's checks and a Cleanup check that reads a Main step. It closes before
LAY-11, which hides the cards those checks would otherwise be seen on;
[ticket](tickets/LAY-20.md), S · 15, decided on 2026-11-07 (Q20.1 = A,
Q20.2 = A), merged in #309.

Deferred to LAY-2 with the reasons in the plan: DIAG-01 descriptions
(`format`), a component summary on the parent board, MCP-13, arrowheads
(#167 (3)), wire-length metrics beyond LAY-01. Mock branches `mock/lay-00-*`
stay on origin because the issues' images point at their commits.

## Sprint: LANG (current; started 2026-11-20)

The language sprint follows LAY, which closed on 2026-11-19 (#296). **The
owner started it on 2026-11-20**, without waiting for the LAY
[retrospective](retrospectives/2026-11-20-lay.md), which was written the
same day. The sequence is the owner's decision 14 = B of 2026-11-19: part
A to 0.2.0 first, the REQ sprint after it. Design and implementation had
been paused since 2026-11-17, except what LAY needed (RUN-03 #389, DIAG-07
#392, LANG-14 #390). The plan, the order and the decisions
are in [sprints/lang.md](sprints/lang.md); the issues carry `sprint:lang`.

- Merged on 2026-11-20: FIX-01 #454 (#485), RUN-01 #126 (#487), LANG-15
  #412 (#486), and beside the sprint #474, #476 (plans and process) and
  #484 (CI).
- Merged since: LANG-13 #386 (#488), LANG-20e [#500](https://github.com/jakubiszon26/apiforge/issues/500) (#504), LANG-05
  #142 (#505), DIAG-06g [#502](https://github.com/jakubiszon26/apiforge/issues/502) (#507), LANG-06 #143 with KTLO-37 #211
  (#508), LAY-45 [#468](https://github.com/jakubiszon26/apiforge/issues/468) (#509), DIAG-06e [#494](https://github.com/jakubiszon26/apiforge/issues/494) (#510).
- To the 0.2.0 release, in merge order: LANG-20 [#481](https://github.com/jakubiszon26/apiforge/issues/481) (`apiforge fix
  --folder`: a whole project in one commit, or nothing) beside DIAG-06a
  #391 (a step moves inside its braces); then DIAG-06d [#493](https://github.com/jakubiszon26/apiforge/issues/493) (with an
  `if`'s braces and across them); then DIAG-06c [#490](https://github.com/jakubiszon26/apiforge/issues/490) (the board
  shows and takes the chain) and DIAG-06f [#496](https://github.com/jakubiszon26/apiforge/issues/496) (paste and
  duplicate); then REL-05 #414. Not a condition of the release: DIAG-06b
  [#491](https://github.com/jakubiszon26/apiforge/issues/491). About 120 owner minutes ahead. The table is in
  [sprints/lang.md](sprints/lang.md#after-the-reviews-and-the-cut-of-2026-11-20).
- **Cut from 0.2.0 by the owner on 2026-11-20** — nobody uses the
  application yet, so the update of a project is one command — and in
  the backlog, to be taken only if users appear: LANG-20b [#497](https://github.com/jakubiszon26/apiforge/issues/497),
  LANG-20c [#498](https://github.com/jakubiszon26/apiforge/issues/498), LANG-20d [#499](https://github.com/jakubiszon26/apiforge/issues/499), LANG-20f [#501](https://github.com/jakubiszon26/apiforge/issues/501), LANG-21a
  [#482](https://github.com/jakubiszon26/apiforge/issues/482), LANG-21b [#495](https://github.com/jakubiszon26/apiforge/issues/495). They carry no sprint label; their ticket
  files stay on their branches.
- Decided by the owner on 2026-11-20 and recorded on the issues: the two
  questions of LANG-13 (#386), LANG-05 (#142), the sixteen questions of
  DIAG-06 (#391), the keys of DIAG-06b (#491), delete, paste and
  duplicate (#494, #496), the chain side to side and how its crossings
  are measured (#468), and the project update (#481: whole, by one
  command, or not at all). Nothing of the first half waits for a
  decision.
- Beside the sprint: [#489](https://github.com/jakubiszon26/apiforge/issues/489), a Windows timing failure of one `mcp08b`
  test, rerun once and noted there until its cause is read.
- After it: LANG-16 #413 (part B of LANG-00), then LANG-01 #127 with
  LANG-09 #353, LANG-08 #352 with LANG-07 #212 and LANG-17, LANG-02 #129
  with LANG-04 #132 and LANG-19, LANG-18, LANG-03 #131.
- Accepted into scope by the owner on 2026-11-20, for the second half —
  analysed and planned there, no design approved yet: LANG-17 [#470](https://github.com/jakubiszon26/apiforge/issues/470)
  a table of cases, or a request based on another (one analysis with
  LANG-16 #413 and LANG-08 #352); LANG-18 [#471](https://github.com/jakubiszon26/apiforge/issues/471) headers and
  authorization shared by a scenario; LANG-19 [#472](https://github.com/jakubiszon26/apiforge/issues/472) a check without
  an invented name, one check for several fields (with LANG-02 #129 and
  LANG-04 #132). Their format questions return to the owner with the
  analyses.
- From LANG-23 [#589](https://github.com/jakubiszon26/apiforge/issues/589) (the reference read by a tester; decided
  2026-11-24, [ticket](tickets/LANG-23.md)): LANG-24 [#591](https://github.com/jakubiszon26/apiforge/issues/591) the reference's
  wording, `ready:sonnet`; LANG-25 [#592](https://github.com/jakubiszon26/apiforge/issues/592) a request's argument read as
  `arg.name` ([ticket](tickets/LANG-25.md), M · `format`, awaits approval and a
  second read); short-circuit `&&`/`||` goes into LANG-01 #127 with
  LANG-09 #353, unary minus into LANG-03 #131; `start` stays required in a
  scenario. Candidates for the owner: LANG-26 [#593](https://github.com/jakubiszon26/apiforge/issues/593) an export always
  readable by name, LANG-27 [#594](https://github.com/jakubiszon26/apiforge/issues/594) a warning for a check written far
  from its step.

## Release 0.2.1: the owner's findings on v0.2.0-rc.1 (closed)

The owner tried the v0.2.0-rc.1 packages on 2026-11-21 and raised four
findings; v0.2.0 was tagged as it was and these go to 0.2.1. LANG-22 is
the planning model's, found while reproducing the second; DIAG-09 is
#399, taken in with DIAG-08. The owner answered the eleven questions the
same day, on the issues. The mock-ups are on `mock/rel-0.2.1`; the
issues carry `sprint:0.2.1`. About 105 owner minutes. The table is in
the order of work.

- Decided the same day on a measurement: the switch the owner chose is
  about 32 px short on a component's header at 960 px, so there the Run
  button is ▶ alone
  ([ticket](tickets/UX-42.md#the-measurement-and-question-12)); a release
  candidate `v0.2.1-rc.1` comes first. DIAG-10, added and decided the same day, is the owner's fifth finding. Nothing waits for a decision.
- Merged: DIAG-09 #399 (#529), UX-41 #519 (#527), UX-42 #520 (#530),
  DEMO-01 #523 (#541), DIAG-08 #521 (#543), LANG-22 #522 (#528), DIAG-10
  #531 (#554). The release candidate's pull request is REL-06 #524; after
  it merges, the owner pushes `v0.2.1-rc.1`, tries its packages and then
  pushes `v0.2.1`.

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#519](https://github.com/jakubiszon26/apiforge/issues/519) | UX-41 The selected step no longer looks like an error: a blue frame, no handles; [ticket](tickets/UX-41.md) | S · 15 min · `visible UI` |
| [#522](https://github.com/jakubiszon26/apiforge/issues/522) | LANG-22 A type check that cannot pass is reported when it is written; [ticket](tickets/LANG-22.md) | S · 10 min |
| [#399](https://github.com/jakubiszon26/apiforge/issues/399) | DIAG-09 Add comparison and Assign to variable work before the first run; [ticket](tickets/DIAG-09.md) | S · 10 min · `visible UI` |
| [#521](https://github.com/jakubiszon26/apiforge/issues/521) | DIAG-08 A check of a field's type in one step from the field; [ticket](tickets/DIAG-08.md); after DIAG-09 | S · 20 min · `visible UI` |
| [#531](https://github.com/jakubiszon26/apiforge/issues/531) | DIAG-10 Output data reads as a tree: a field on one line; [ticket](tickets/DIAG-10.md); after DIAG-08 | S · 20 min · `visible UI`, `removal` |
| [#523](https://github.com/jakubiszon26/apiforge/issues/523) | DEMO-01 The first-start project shows the 0.2.0 language; [ticket](tickets/DEMO-01.md); after LANG-22 and DIAG-08 only in that its scenario is what they are tried on | M · 30 min · `default`, `visible UI` |
| [#520](https://github.com/jakubiszon26/apiforge/issues/520) | UX-42 The run mode is a switch in the header; [ticket](tickets/UX-42.md) | S · 20 min · `visible UI`, `removal` |
| [#524](https://github.com/jakubiszon26/apiforge/issues/524) | REL-06 v0.2.1, a candidate first; [ticket](tickets/REL-06.md); last | S · 15 min |

AB-01 [#537](https://github.com/jakubiszon26/apiforge/issues/537), v0.1.0 against v0.2.0 on eight tester's tasks driven by
agents ([ticket](tickets/AB-01.md)), runs beside these tickets: the owner
approved the method and the timing on 2026-11-21. Round 1 (T1, T2) is
done and classed: no class 1, R1 #547 class 2 into REQ, R2 #548 class 3.
Rounds 2 to 4 compare v0.1.0 with `v0.2.1-rc.2` (owner, 2026-11-23);
round 2 is `ready:opus`; it changes nothing in the product.

## Next sprint: REQ (not started)

Request and results. The owner's run of 2026-11-19 showed that a test made
of independent requests is read and edited in the wrong places: results
that do not say what was checked, a body eight sections down a narrow
panel, a response folded behind clicks, and no view whose unit is one
request. The findings, causes, mock-ups and decisions are in
[`docs/sprints/req.md`](sprints/req.md); the issues carry `sprint:req`.
It follows LANG part A and is designed for the written-order chain of
LANG-13. FIX-01 merged on 2026-11-20 (#485), ahead of the sprint, as a
case for LANG-13's fix. Three more of its tickets do not depend on the
language and are ready now (DIAG-02 and REP-08, then REP-07); they wait
for the owner to open the sprint or to pull them forward.

Capacity: **235** owner review minutes over the 12 tickets decided on
2026-11-19, and about **75** more for the four proposals the owner
accepted into scope on 2026-11-20 (REQ-07 to REQ-10), whose estimates are
the proposals' own until each is analysed: about 310 over 16. Exit
criteria:

- [ ] A check reads the same on its card, on a folded line, in the
      Inspector and in Run results, and says what it compares (DIAG-02).
- [ ] After a run on `labels` the list says which request failed and which
      check, without opening a row; a response is read without a click
      (REP-07, REP-08).
- [ ] A scenario is read as a numbered list of steps with their checks
      (REQ-01).
- [ ] A request is edited with its response beside it: the body in an
      editor with line numbers and validity, headers in a table,
      authorization from a list (REQ-02a, REQ-02b, REQ-03; closes DIAG-04).
- [ ] *Run to here* and *Send this request alone* work from that view; a
      live request is sent only under the permission an ordinary live run
      needs (REQ-04, REQ-05).
- [ ] The mandatory scenario — POST, the id from the response, GET with
      that id, an assertion, DELETE — is built and run twice in the window
      with no hand editing of the `.afs` (owner, 2026-11-21; REQ-06,
      [ticket](tickets/REQ-06.md)); agents run it first
      ([A/B rounds](process/ab-testing.md)).
- [ ] The owner does the task of REQ-06 on a release build, and the
      retrospective is written.

| Issue | Ticket | Estimate · risk | State |
| --- | --- | --- | --- |
| [#454](https://github.com/jakubiszon26/apiforge/issues/454) | FIX-01 A reference scenario of independent negative requests (`labels`) | S · 5 | merged in #485 (2026-11-20) |
| [#145](https://github.com/jakubiszon26/apiforge/issues/145) | DIAG-02 A check reads the same everywhere and says what it compares | M · 15 · `visible UI`, `removal` | `ready:opus`; not open together with LAY-45, DIAG-06a or DIAG-06c (`canvas/`) |
| [#455](https://github.com/jakubiszon26/apiforge/issues/455) | REP-08 Run results: a value opens expanded; Tree \| JSON; Expand all | S · 15 · `visible UI`, `default` | `ready:opus` · criterion 7: the row is DIAG-10's ([#531](https://github.com/jakubiszon26/apiforge/issues/531)) |
| [#381](https://github.com/jakubiszon26/apiforge/issues/381) | REP-07 Run results: a request with its checks under it | M · 25 · `visible UI`, `default` | `ready:opus` · after DIAG-02, REP-08 and DIAG-12 ([#533](https://github.com/jakubiszon26/apiforge/issues/533)), whose index it uses |
| [#533](https://github.com/jakubiszon26/apiforge/issues/533) | DIAG-12 A field of a response that a check already reads is marked | M · 20 · `visible UI` | decided 2026-11-23 ([ticket](tickets/DIAG-12.md)): the ✓ filled, checks only; `ready:opus` · after REP-08 merges, before REP-07 |
| [#547](https://github.com/jakubiszon26/apiforge/issues/547) | DIAG-15 The Checks tab of a step can add a check | S · 15 · `visible UI` | class 2 of AB-01 round 1; decided 2026-11-23 ([ticket](tickets/DIAG-15.md)): a button, then a menu of fields; `ready:opus` |
| [#550](https://github.com/jakubiszon26/apiforge/issues/550) | DIAG-16 Choose a response field takes a typed path | S · 10 · `visible UI` | decided 2026-11-23 ([ticket](tickets/DIAG-16.md)): a row *Use "…"*; `ready:opus`; the sprint is the owner's to name |
| [#456](https://github.com/jakubiszon26/apiforge/issues/456) | REQ-00 The Steps view and the request beside its response: captures and decisions; [ticket](tickets/REQ-00.md) | S · 30 · `visible UI`, `default` | decided 2026-11-24: every board A, P4, the rules ruled |
| [#457](https://github.com/jakubiszon26/apiforge/issues/457) | REQ-01a Steps reads: the scenario as a numbered list; [ticket](tickets/REQ-01a.md) | M · 30 · `visible UI` | `ready:opus` · after REP-07a ([#588](https://github.com/jakubiszon26/apiforge/pull/588)) · owns the rules of SENDS |
| [#599](https://github.com/jakubiszon26/apiforge/issues/599) | REQ-04a The engine runs through a step, and one request alone with its checks; [ticket](tickets/REQ-04a.md) | M · 10 | `ready:opus` · in parallel with REQ-01a |
| [#600](https://github.com/jakubiszon26/apiforge/issues/600) | REQ-01b Steps adds and moves a step; the empty scenario; closes WS-04 [#548](https://github.com/jakubiszon26/apiforge/issues/548); [ticket](tickets/REQ-01b.md) | M · 20 · `visible UI`, `default` | `ready:opus` · after REQ-01a, DIAG-22 and WS-05b |
| [#458](https://github.com/jakubiszon26/apiforge/issues/458) | REQ-02a The selected request and its response side by side; [ticket](tickets/REQ-02a.md) | M · 30 · `visible UI`, `default` | `ready:opus` · after REQ-01a, REP-07b and REQ-15 |
| [#549](https://github.com/jakubiszon26/apiforge/issues/549) | REQ-14 A step is moved between Main and Cleanup, with its checks; [ticket](tickets/REQ-14.md) | M · 15 · `visible UI` | `ready:opus` · after REQ-01b and DIAG-22 (`canvas/menu.rs`) |
| [#459](https://github.com/jakubiszon26/apiforge/issues/459) | REQ-02b The body editor; closes DIAG-04 [#365](https://github.com/jakubiszon26/apiforge/issues/365); [ticket](tickets/REQ-02b.md) | M · 20 · `visible UI` | `ready:opus` · after REQ-02a |
| [#460](https://github.com/jakubiszon26/apiforge/issues/460) | REQ-03 Headers as a table; authorization from a list; [ticket](tickets/REQ-03.md) | M · 15 · `visible UI` | `ready:opus` · after REQ-02a |
| [#461](https://github.com/jakubiszon26/apiforge/issues/461) | REQ-04b Run to here; [ticket](tickets/REQ-04b.md) | M · 20 · `visible UI` | `ready:opus` · after REQ-02a and REQ-04a |
| [#462](https://github.com/jakubiszon26/apiforge/issues/462) | REQ-05 Send this request alone; [ticket](tickets/REQ-05.md) | M · 25 · `visible UI`, `default` | `ready:opus` · after REQ-04b |
| [#601](https://github.com/jakubiszon26/apiforge/issues/601) | REQ-16 Send alone can send a secret the last run received | S–M · `default` | proposed 2026-11-24, `decision-needed` |
| [#464](https://github.com/jakubiszon26/apiforge/issues/464) | REQ-07 A request in and out as cURL; a failed check copied as a report | S–M · 15 · `visible UI` | accepted into scope 2026-11-20; to analyse; after REP-07 and REQ-02a |
| [#465](https://github.com/jakubiszon26/apiforge/issues/465) | REQ-08 A new case duplicated from an existing one; run the failed ones again | S and M · 30 · `visible UI` | accepted into scope 2026-11-20; to analyse; after REQ-01 and REQ-04 |
| [#477](https://github.com/jakubiszon26/apiforge/issues/477) | REQ-09 Hints for repetition: the same host or header in every request, each with a fix | S · 10 · `visible UI` | accepted into scope 2026-11-20; to analyse; the header half after LANG-18 |
| [#478](https://github.com/jakubiszon26/apiforge/issues/478) | REQ-10 A live response saved as the sample; Expect this response | M · 20 · `visible UI` | accepted into scope 2026-11-20; to analyse; after REP-07 and REQ-02a |
| [#538](https://github.com/jakubiszon26/apiforge/issues/538) | REQ-11 A value of a response is used in a later request by choosing it | M · 20 · `visible UI` | in scope 2026-11-21; to analyse, with an A/B round; after DIAG-08 |
| [#539](https://github.com/jakubiszon26/apiforge/issues/539) | REQ-12 A check made from a field offers what to compare with | S–M · 15 · `visible UI`, `default` | in scope 2026-11-21; to analyse, with an A/B round; after DIAG-08 |
| [#540](https://github.com/jakubiszon26/apiforge/issues/540) | REQ-13 The window writes only what was set; [ticket](tickets/REQ-13.md) | S–M · 15 · `default` | in scope 2026-11-21; three questions open |
| [#574](https://github.com/jakubiszon26/apiforge/issues/574) | WS-05a A scenario's title above its board, edited there; [ticket](tickets/WS-05a.md) | M · 15 · `visible UI` | decided 2026-11-24 (B, editable); `ready:opus` |
| [#596](https://github.com/jakubiszon26/apiforge/issues/596) | WS-05b Files lists scenarios and components by title; [ticket](tickets/WS-05b.md) | M · 15 · `visible UI` | decided 2026-11-24 (B); `ready:opus` · after WS-05a |
| [#576](https://github.com/jakubiszon26/apiforge/issues/576) | REQ-15 A request with no sample response leads to where one is set; [ticket](tickets/REQ-15.md) | S · 10 · `visible UI` | decided 2026-11-24 (A); `ready:opus` · after REP-07a and REP-07b |
| [#607](https://github.com/jakubiszon26/apiforge/issues/607) | REQ-17a Tries are held beside the project session and edited by its own code; [ticket](tickets/REQ-17a.md) | M · 10 · the session's edit code | decided as a need 2026-11-24; `ready:opus` |
| [#603](https://github.com/jakubiszon26/apiforge/issues/603) | REQ-17b One request tried without making a test of it: New request, its tab, Send; [ticket](tickets/REQ-17b.md) | M · 25 · `visible UI`, `permission` | decided 2026-11-24 (1 A, 3 A, 5 A); `ready:opus` · after REQ-17a, REQ-02a, REQ-05, REQ-16 |
| [#608](https://github.com/jakubiszon26/apiforge/issues/608) | REQ-17c Tries are listed in Files and kept with the person across a restart; [ticket](tickets/REQ-17c.md) | M · 15 · `visible UI`, `permission` | decided 2026-11-24 (2 A, 5 A; a typed secret is not written); `ready:opus` · after REQ-17b |
| [#609](https://github.com/jakubiszon26/apiforge/issues/609) | REQ-17d A try is kept: added to a scenario or saved as a new one; [ticket](tickets/REQ-17d.md) | M · 15 · `visible UI` | decided 2026-11-24 (4 A); `ready:opus` · after REQ-17b, REQ-01b, REQ-18 |
| [#604](https://github.com/jakubiszon26/apiforge/issues/604) | REQ-18 A request saved as a component in one action; [ticket](tickets/REQ-18.md) | M · 15 · `visible UI`, `default` | decided 2026-11-24 (1 A, 2 A, 3 B, 4 A); `ready:opus` · after REQ-02a, REQ-05 |
| [#601](https://github.com/jakubiszon26/apiforge/issues/601) | REQ-16 Send alone can send a secret the last run received; [ticket](tickets/REQ-16.md) | M · 10 · `permission`, `default` | decided 2026-11-24 (A); `ready:opus` · after REQ-05 |
| [#610](https://github.com/jakubiszon26/apiforge/issues/610) | REQ-19 The demo API can write a log of the requests it received | S · 5 | taken by the owner 2026-11-24; `ready:opus`; tooling for REQ-06 part 3 |
| [#463](https://github.com/jakubiszon26/apiforge/issues/463) | REQ-06 Acceptance: the mandatory scenario and the owner's own run; [ticket](tickets/REQ-06.md) | S · 10 | after the rest |

REQ-07 to REQ-10 are the planning model's proposals
([plan, section 5](sprints/req.md#5-proposals-of-the-planning-model)).
Accepted means in scope: each still gets its analysis, its mock-up and the
owner's decisions before any label. GEN-01 [#473](https://github.com/jakubiszon26/apiforge/issues/473), suggested negative
cases of a field, is accepted too and is in no sprint: it writes rows of a
table of cases and is analysed after LANG-17. UX-19 [#227](https://github.com/jakubiszon26/apiforge/issues/227) edits the same list as REP-07 and merges beside it,
not with it.

## Later

Feature work resumes after KTLO exits. Candidates, unestimated:

- The update of a project beyond one command — converting in parts with
  a record, an MCP tool, the window's *Update the project*: LANG-20b
  [#497](https://github.com/jakubiszon26/apiforge/issues/497), LANG-20c [#498](https://github.com/jakubiszon26/apiforge/issues/498), LANG-20d [#499](https://github.com/jakubiszon26/apiforge/issues/499), LANG-20f [#501](https://github.com/jakubiszon26/apiforge/issues/501),
  LANG-21a [#482](https://github.com/jakubiszon26/apiforge/issues/482), LANG-21b [#495](https://github.com/jakubiszon26/apiforge/issues/495). Cut from 0.2.0 on 2026-11-20;
  analysed and reviewed, each with its ticket file on its branch; taken
  only if users with 0.1.0 projects appear.
- Three proposals of DIAG-10 the owner took into the backlog on
  2026-11-21, each to analyse with a mock-up: DIAG-11 [#532](https://github.com/jakubiszon26/apiforge/issues/532),
  *Copy value* and *Copy path* on a row of a response, with REP-04
  [#148](https://github.com/jakubiszon26/apiforge/issues/148); DIAG-13 [#534](https://github.com/jakubiszon26/apiforge/issues/534), a filter by name above the tree;
  DIAG-14 [#535](https://github.com/jakubiszon26/apiforge/issues/535), the component dialog's path picker takes the
  row of Output data.
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
[LAY sprint](#sprint-lay-2026-11-05-to-2026-11-19-closed) above. The rows below
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
