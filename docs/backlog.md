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
end to end once. No new features. Exit criteria:

- [ ] Migration of the owner's real workspace works, or is switched off by an
      owner decision (KTLO-03, #5).
- [ ] No ignored test without a ticket; the identity checkpoint decision is
      made (KTLO-04, #6).
- [ ] The block library in the folder mode is accepted by the owner through
      the full process: mock-up, acceptance tests, QA report (KTLO-05a #7,
      KTLO-05b #8).
- [ ] The process documents were followed once end to end, and amended where
      the pilot required it.
- [ ] The regression smoke list has run on Linux and on Windows (KTLO-07, #10).
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
| [#8](https://github.com/jakubiszon26/apiforge/issues/8) | KTLO-05b Block library — implementation (pilot of the full procedure) | M · 45 min · `visible UI` |
| [#9](https://github.com/jakubiszon26/apiforge/issues/9) | KTLO-06 Legacy JSON mode sunset: parity list and deprecation notice | S (docs) · 20 min · `removal` |
| [#10](https://github.com/jakubiszon26/apiforge/issues/10) | KTLO-07 Regression smoke list: first run on Linux and Windows | S · 45 min |
| [#11](https://github.com/jakubiszon26/apiforge/issues/11) | KTLO-08 CI uploads QA captures per platform; [ticket](tickets/KTLO-08.md) | S · 10 min; actual 471 lines (M), 1 round |
| [#12](https://github.com/jakubiszon26/apiforge/issues/12) | KTLO-09 Orphaned code audit | S · 10 min |
| [#13](https://github.com/jakubiszon26/apiforge/issues/13) | KTLO-10 KTLO retrospective | S (docs) · 30 min |

## Later

Feature work resumes after KTLO exits. Candidates, unestimated:

- [#14](https://github.com/jakubiszon26/apiforge/issues/14) KTLO-11 Scenario
  navigation in Diagram mode (from the KTLO-05a decision).
- Removal of the legacy JSON mode once the KTLO-06 parity list is closed.
- Roadmap M7 editor features ([roadmap](roadmap.md)).
- Windows release gates: interactive native QA and directory durability
  ([Stage II plan](stage-ii-code-workspace.md#delivery-and-acceptance)).
