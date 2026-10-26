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
of owner review. KTLO-20 is the last ticket added to this sprint; its scope
is closed. Exit criteria:

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
- [ ] The regression smoke list has run on Linux and on Windows (KTLO-07, #10).
- [ ] The two interface findings the owner raised on the mock-ups — one header
      above the board (KTLO-12, #16) and an editor without a box (KTLO-13,
      #17) — are accepted through the same mock-up → test → report path.
- [ ] The five interface wishes of 2026-10-26 are decided on their mock-ups
      (KTLO-17 #42, KTLO-11 #14, KTLO-18 #43, KTLO-19 #44, KTLO-20 #46); those
      approved are accepted or carried into the next sprint with their state
      recorded.
- [ ] The KTLO retrospective is written and its actions are issues (KTLO-10, #13).

If the criteria are not met by 2026-11-08, the sprint is extended; feature
work does not start.

| Issue | Ticket | Estimate |
| --- | --- | --- |
| — | KTLO-01 Process documents — merged in #1 | M (docs) · 30 min; estimated S, ~730 lines written |
| — | KTLO-02 Stage II retrospective — merged in #1; facts corrected in #2 | S (docs) · 15 min |
| [#5](https://github.com/jakubiszon26/apiforge/issues/5) | KTLO-03 Migration refuses a real six-project workspace | M · 30 min · `migration` |
| [#6](https://github.com/jakubiszon26/apiforge/issues/6) | KTLO-04 Identity checkpoint at initialise, register_existing and import | S–M · 30 min · `format` |
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
| [#46](https://github.com/jakubiszon26/apiforge/issues/46) | KTLO-20 A sleeping dog keeps the empty board company in Diagram and Split; [ticket](tickets/KTLO-20.md) — the last ticket added to this sprint; animated (owner decision) | M · 30 min · `visible UI` |

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
- Removal of the legacy JSON mode once the KTLO-06 parity list is closed.
- Roadmap M7 editor features ([roadmap](roadmap.md)).
- Windows release gates: interactive native QA and directory durability
  ([Stage II plan](stage-ii-code-workspace.md#delivery-and-acceptance)).
