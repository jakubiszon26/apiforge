# Backlog

Tickets are [GitHub issues](https://github.com/jakubiszon26/apiforge/issues),
one per ticket, titled `<ID>: <goal>` and labelled as the
[development process](process/README.md#labels) says. Their state — open,
closed, `decision-needed`, `awaiting-acceptance` — lives there and nowhere
else. This file names the current sprint, its exit criteria and the issues
in it, and keeps the record of finished sprints.

At most three pull requests await owner acceptance at a time.

## Sprint: KTLO (2026-10-26 to 2026-11-08)

Keep the lights on: stabilise what Stage II shipped and run the new process
end to end once. No new features, with one exception the owner made on
2026-10-26: five interface wishes for the folder window (KTLO-17, KTLO-11,
KTLO-18, KTLO-19, KTLO-20) join the sprint and go through the full path,
mock-up → decision → acceptance tests → QA report. They add about 115 minutes
of owner review. A sixth wish was added later the same day, after KTLO-20 had
been called the last: running a component from its own tab (KTLO-23, two parts,
65 further owner minutes), an owner decision that reopened the sprint's scope
for that one ticket, and once more for a seventh, KTLO-24 (#57, S · 15 min,
the open-files bar hides itself). Exit criteria:

- [ ] Migration of the owner's real workspace works, or is switched off by an
      owner decision (KTLO-03, #5).
- [ ] No ignored test without a ticket; the identity checkpoint decision is
      made (KTLO-04, #6).
- [x] The block library in the folder mode is accepted by the owner through
      the full process: mock-up, acceptance tests, QA report (KTLO-05a #7,
      KTLO-05b #8; accepted 2026-10-26). Follow-ups: KTLO-05c #22,
      KTLO-05d #23.
- [ ] The process documents were followed once end to end, and amended where
      the pilot required it.
- [x] The regression smoke list has run on Linux and on Windows (KTLO-07, #10;
      closed 2026-10-26).
- [x] The two interface findings the owner raised on the mock-ups — one header
      above the board (KTLO-12, #16) and an editor without a box (KTLO-13,
      #17) — are accepted through the same mock-up → test → report path
      (KTLO-13 merged in #39, KTLO-12 in #41 and #47).
- [ ] The five interface wishes of 2026-10-26 are decided on their mock-ups
      (KTLO-17 #42, KTLO-11 #14, KTLO-18 #43, KTLO-19 #44, KTLO-20 #46); those
      approved are accepted or carried into the next sprint with their state
      recorded.
- [ ] A component runs from its tab the way a scenario does: the mock-up is
      decided and the two parts are accepted, or carried into the next sprint
      with their state recorded (KTLO-23a #53, KTLO-23b #54).
- [ ] The open-files bar hides itself when no file is open (KTLO-24, #57),
      decided on its mock-up and accepted, or carried into the next sprint
      with its state recorded.
- [ ] The KTLO retrospective is written and its actions are issues (KTLO-10, #13).

If the criteria are not met by 2026-11-08, the sprint is extended; feature
work does not start.

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

An MCP server: a model works on a project without the window. Analysed
2026-10-27 ([MCP-00](tickets/MCP-00.md)) at the owner's request of
2026-10-26: a `mcp` subcommand of the `apiforge` binary over stdio, so that
Claude Code or another MCP client reads, checks, edits and runs the scenarios
of a folder project through API Forge's own session — the same files and
bytes the window writes. The owner called it a sprint of its own and took
the nine decisions of MCP-00 on 2026-10-27
([#59](https://github.com/jakubiszon26/apiforge/issues/59)): 1–8 the
recommended option (writes, invalid text, publishing, live requests, the lock
beside an open window, the window's wording, the reference, the default
folder), and 9 = B: this sprint follows KTLO and LAY follows it. Exit
criteria:

- [ ] In the demo shop folder, `claude mcp add apiforge -- apiforge mcp`
      lets a model list the project, read and check every source with line
      and column, print, run a simulation and check a proposed text, with
      nothing written to the project (MCP-01, MCP-03).
- [ ] The language reference a model receives is a tested document whose
      examples parse, format unchanged and lower (MCP-02).
- [ ] With the owner's decisions 1–3 taken, the model saves, registers and
      publishes through the server under the writer lock, never while the
      window holds the project, and the window follows what it wrote
      (MCP-04, MCP-05).
- [ ] Live requests are off unless the person starts the server with
      `--allow-live`; secrets never pass through a tool (MCP-06).
- [ ] The window names who holds the project and becomes writable when they
      leave, through mock-up, acceptance tests and QA report (MCP-07).

| Issue | Ticket | Estimate |
| --- | --- | --- |
| [#59](https://github.com/jakubiszon26/apiforge/issues/59) | MCP-00 Analysis and the owner's decisions; [ticket](tickets/MCP-00.md) | S (docs) · 30 min plus the discussion · decisions taken 2026-10-27 |
| [#60](https://github.com/jakubiszon26/apiforge/issues/60) | MCP-01 `apiforge mcp` answers Claude Code: overview, read, check, print; [ticket](tickets/MCP-01.md) | M · 20 min · `default` |
| [#61](https://github.com/jakubiszon26/apiforge/issues/61) | MCP-02 A language reference the model reads from the server; [ticket](tickets/MCP-02.md) | M (docs) · 30 min |
| [#62](https://github.com/jakubiszon26/apiforge/issues/62) | MCP-03 Run a simulation and check a proposed text from the server; [ticket](tickets/MCP-03.md); after MCP-01 | S · 15 min |
| [#63](https://github.com/jakubiszon26/apiforge/issues/63) | MCP-04 Save a scenario or component from the server; [ticket](tickets/MCP-04.md); decisions 1, 2, 5 | M · 30 min |
| [#64](https://github.com/jakubiszon26/apiforge/issues/64) | MCP-05 Publish a component from the server; [ticket](tickets/MCP-05.md); decision 3, after MCP-04 | S · 15 min |
| [#65](https://github.com/jakubiszon26/apiforge/issues/65) | MCP-06 Live requests from the server behind `--allow-live`; [ticket](tickets/MCP-06.md); decision 4, after MCP-03 | S · 20 min |
| [#66](https://github.com/jakubiszon26/apiforge/issues/66) | MCP-07 The window names who holds the project and becomes writable when they leave; [ticket](tickets/MCP-07.md); decision 6, mock-up required | S · 20 min · `visible UI`, `default` |
| [#57](https://github.com/jakubiszon26/apiforge/issues/57) | KTLO-24 The bar with the open files hides itself when no file is open; [ticket](tickets/KTLO-24.md) | S · 15 min · `visible UI`, `default` |
| [#76](https://github.com/jakubiszon26/apiforge/issues/76) | KTLO-25 The demo Product component carries a type warning | S · 5 min |

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
