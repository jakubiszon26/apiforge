# Development process

This directory defines how work on API Forge is planned, estimated, built,
verified and reviewed. It applies equally to people and to coding agents. It
was introduced after the [Stage II retrospective](../retrospectives/2026-10-25-stage-ii.md),
whose findings every rule below answers.

| Document | Covers |
| --- | --- |
| [Planning](planning.md) | Ticket template, analysis, mock-ups, who decides what |
| [Estimation](estimation.md) | Size classes, review time, risk flags, splitting |
| [Testing](testing.md) | Acceptance tests first, the QA report, the regression smoke list |
| [Retrospective](retrospective.md) | The sprint review and its three actions |
| [Backlog](../backlog.md) | The board: every ticket, its state and its estimate |
| [Development guide](../development.md) | Toolchain, verification commands, branch rules |
| [GUI testing](../gui-testing.md) | Interaction tests, design review captures, native probes |

## Lifecycle of a ticket

```text
Backlog → Analysis → Estimate → Owner approval → Acceptance test → Implementation
        → QA report → Owner acceptance → Merge to main → Retrospective
```

1. **Backlog.** A ticket is a row in [`docs/backlog.md`](../backlog.md) with an
   ID (`KTLO-03`, `S1-07`, …), a one-line goal in the owner's words and a size.
2. **Analysis.** Written for the owner, one page at most: what changes on
   screen, what disappears, what migrates, what the acceptance criteria are.
   A change that touches the interface includes a mock-up. Technical detail
   goes to an appendix or an ADR. See [planning](planning.md).
3. **Estimate.** Size, owner review minutes and risk flags, recorded in the
   ticket. An `L` ticket is split before it is scheduled.
4. **Owner approval.** The owner approves the analysis, or sends it back. No
   code before approval, except a throwaway branch that exists only to produce
   the mock-up.
5. **Acceptance test.** The acceptance criteria become failing tests, committed
   on their own before the implementation.
6. **Implementation.** One branch from `main`, one pull request, within the
   size class. The verification commands of the development guide pass.
7. **QA report.** A functional report with screenshots per platform and an
   explicit list of what was not verified. See [testing](testing.md).
8. **Owner acceptance.** The owner reads the report and, when they choose,
   builds and uses the change locally. Only the owner merges to `main`.
9. **Retrospective.** At the end of each sprint, with the measures from
   [retrospective](retrospective.md).

## Who decides

The owner decides the product; the agent decides how it is built, and says in
one sentence what each technical decision means for the person using the
application.

**Always the owner's decision.** The agent recommends in a paragraph at most,
stops, and asks:

| Category | Examples |
| --- | --- |
| Removing or hiding a visible feature or screen | A panel absent from a new mode; a menu entry gone |
| Changing default behaviour or the default start | Which window opens; what a shortcut does; panel layout |
| Backward compatibility and migration of user data | Supporting an old format; migrating automatically; breaking |
| The project file format | `.afs` syntax, the manifest, checkpoints, anything Git tracks |

**The agent's decision**, recorded where it is made (commit, ADR, ticket):
internal architecture, crate boundaries, test strategy, refactoring, tooling.
A decision in this class that still changes what the owner sees is reported in
the ticket before implementation.

## Limits that keep review possible

- **Work in progress:** at most three pull requests awaiting owner acceptance.
  A fourth is not started; the agent picks up review feedback instead.
- **Size:** `S` ≤ 300 changed lines, `M` ≤ 800, `L` is split. Generated
  fixtures and lock files are not counted; tests and docs are.
- **Branches:** every branch starts at `main`. A branch is never based on an
  unmerged branch, and there is no integration branch that collects several
  tickets before review.
- **Ignored tests:** `#[ignore = "KTLO-04: …"]` names the ticket that will
  resolve it; an ignore without a ticket is a defect.

## Which model does what

| Stage | Model | Note |
| --- | --- | --- |
| Planning and analysis: tickets, mock-ups, estimates, questions to the owner, retrospectives | Fable | Always. If Fable is unavailable, Opus may plan and the ticket records it |
| Programming: code, tests, refactoring, QA reports | Opus 5.5 or newer | Never a smaller model for code |
| Mechanical edits: a typo, a one-line change, formatting | Sonnet | Changes that contain no decision |

Subagents follow the same split: a planning agent delegates implementation to an
Opus subagent and trivia to a Sonnet subagent; an implementing agent never
delegates analysis to a smaller model. The pull request records which model did
the analysis and which did the implementation.

## What the repository contains

Tickets, decisions, evidence and the code. It does not contain the content of
conversations between the owner and an agent: a decision is recorded as the
decision and its reason, not as who said what.
