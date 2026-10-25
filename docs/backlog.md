# Backlog

The board for API Forge. One row per ticket; the columns follow the
[development process](process/README.md). A ticket above `S` or with a risk
flag has a file in `docs/tickets/`. Work in progress is limited to three
tickets awaiting owner acceptance.

Estimate: size · owner review minutes · risk flags
([estimation](process/estimation.md)). Actual values are added when a ticket is
done.

## Sprint: KTLO (2026-10-26 to 2026-11-08)

Keep the lights on: stabilise what Stage II shipped and run the new process
end to end once. No new features. Exit criteria:

- [ ] Migration of the owner's real workspace works, or is switched off by an
      owner decision (KTLO-03).
- [ ] No ignored test without a ticket; the identity checkpoint decision is
      made (KTLO-04).
- [ ] The block palette in the folder mode is accepted by the owner through
      the full process: mock-up, acceptance tests, QA report (KTLO-05a/b).
- [ ] The process documents were followed once end to end, and amended where
      the pilot required it.
- [ ] The regression smoke list has run on Linux and on Windows (KTLO-07).
- [ ] The KTLO retrospective is written and its actions are tickets (KTLO-10).

If the criteria are not met by 2026-11-08, the sprint is extended; feature
work does not start.

### Done

| ID | Ticket | Estimate | Actual | PR |
| --- | --- | --- | --- | --- |

### Awaiting owner acceptance (≤ 3)

| ID | Ticket | Estimate | PR |
| --- | --- | --- | --- |
| KTLO-01 | Process documents: lifecycle, planning, estimation, testing, retrospective, board, PR template | M (docs) · 30 min — estimated S, ~730 lines written | `docs/process` |
| KTLO-02 | Stage II retrospective in the repository | S (docs) · 15 min | `docs/process` |

### In progress

| ID | Ticket | Estimate | Branch |
| --- | --- | --- | --- |

### Ready

| ID | Ticket | Estimate | Owner decision |
| --- | --- | --- | --- |
| KTLO-03 | Migration refuses a real six-project workspace: isolate the cause with an anonymised fixture, add the acceptance test, fix | M · 30 min · `migration` | How a refused migration should behave |
| KTLO-05a | Block palette in the folder mode: analysis and mock-up (pilot of the planning procedure) | S · 20 min · `visible UI` | Mock-up approval |
| KTLO-08 | CI: upload the Linux and Windows captures of every push as artifacts, named by branch, so QA reports can cite Windows evidence | S · 10 min | — |

### Backlog

| ID | Ticket | Estimate | Owner decision |
| --- | --- | --- | --- |
| KTLO-04 | Identity checkpoint at `initialise`/`register_existing`/`import`: recommendation, decision, un-ignore the acceptance test | S–M · 30 min · `format` | Seal at registration, at first analysis, or at first save |
| KTLO-05b | Block library panel in the folder mode: acceptance tests, implementation reusing `library.rs`, the canvas drop handling and `Command::AddNode`, QA report (pilot of the full procedure) | M · 45 min · `visible UI` | Acceptance |
| KTLO-06 | Legacy JSON mode sunset: parity list folder vs legacy, deprecation notice in README and roadmap | S (docs) · 20 min · `removal` | Approve the parity list |
| KTLO-07 | Regression smoke list: first run on Linux (agent) and Windows (owner's build), results recorded | S · 45 min | Run the Windows pass |
| KTLO-09 | Orphaned code audit: list every symbol reachable only from the legacy window; output is the list, not a removal | S · 10 min | — |
| KTLO-10 | KTLO retrospective | S (docs) · 30 min | Participation |

## Later

Feature work resumes after KTLO exits. Candidates, unestimated:

- Removal of the legacy JSON mode once the KTLO-06 parity list is closed.
- Roadmap M7 editor features ([roadmap](roadmap.md)).
- Windows release gates: interactive native QA and directory durability
  ([Stage II plan](stage-ii-code-workspace.md#delivery-and-acceptance)).
