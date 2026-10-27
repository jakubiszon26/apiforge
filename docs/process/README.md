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
| [Autonomy](autonomy.md) | Which tickets another model may take unattended, how it picks one, what it escalates, who merges |
| [Backlog](../backlog.md) | The sprint: its goal, exit criteria and the issues in it |
| [Issues](https://github.com/jakubiszon26/apiforge/issues) | The tickets: one issue each, with state, estimate and discussion |
| [Development guide](../development.md) | Toolchain, verification commands, branch rules |
| [GUI testing](../gui-testing.md) | Interaction tests, design review captures, native probes |

## Lifecycle of a ticket

```text
Backlog → Analysis → Estimate → Owner approval → Acceptance test → Implementation
        → QA report → Owner acceptance → Merge to main → Retrospective
```

1. **Backlog.** A ticket is a GitHub issue titled `<ID>: <goal>` (`KTLO-03`,
   `S1-07`, …) with the goal in the owner's words, the estimate and the labels
   below. The issue is where the ticket's state lives: open or closed, and
   `decision-needed` or `awaiting-acceptance` while it waits for the owner.
   [`docs/backlog.md`](../backlog.md) names the sprint, its exit criteria and
   the issues in it; it does not repeat their state.
2. **Analysis.** Written for the owner, one page at most: what changes on
   screen, what disappears, what migrates, what the acceptance criteria are.
   A change that touches the interface includes a mock-up. Technical detail
   goes to an appendix or an ADR. The analysis is the first commit of the
   ticket's draft pull request, which then carries everything below
   ([one result, one pull request](planning.md#one-acceptable-result-one-pull-request)).
   See [planning](planning.md).
3. **Estimate.** Size by complexity, uncertainty, regression potential and
   verification cost; owner review minutes; risk flags, recorded in the
   ticket. An `L` ticket is split before it is scheduled; changed lines are
   recorded, never a limit ([estimation](estimation.md)).
4. **Owner approval.** The owner approves the analysis, or sends it back. No
   code before approval, except a throwaway branch that exists only to produce
   the mock-up.
5. **Acceptance test.** The acceptance criteria become failing tests, committed
   on their own before the implementation.
6. **Implementation.** On the same branch from `main`, in the same pull
   request. The verification commands of the development guide pass. A ticket
   labelled `ready:sol` or `ready:sonnet` may be taken from here by that model
   under [autonomy](autonomy.md).
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

- **Work in progress:** at most **four** pull requests awaiting owner
  acceptance, and the sum of their estimated review minutes at most **60**.
  Whichever limit is reached first stops the next start; the agent picks up
  review feedback instead. The numbers are the MCP sprint's experiment
  (KTLO-30, from three and no minute cap in KTLO) and are measured again at
  its retrospective: wait time, rounds after acceptance, findings after
  acceptance.
- **Size:** `S`, `M` or `L` by [complexity and risk](estimation.md#size);
  `L` is split. Changed lines are recorded in the pull request, not limited.
- **Branches:** every branch starts at `main`. A branch is never based on an
  unmerged branch, and there is no integration branch that collects several
  tickets before review.
- **Ignored tests:** `#[ignore = "KTLO-04: …"]` names the ticket that will
  resolve it; an ignore without a ticket is a defect.

## Labels

| Label | Meaning |
| --- | --- |
| `sprint:<name>` | The sprint the issue belongs to |
| `size:S`, `size:M`, `size:L` | The [size class](estimation.md#size); `L` is not scheduled |
| `risk:format`, `risk:migration`, `risk:default`, `risk:visible-ui`, `risk:removal` | The [risk flags](estimation.md#risk-flags) |
| `decision-needed` | Waits for an owner decision named in the issue |
| `awaiting-acceptance` | A pull request is open and waits for the owner |
| `ready:sol`, `ready:sonnet` | Approved, decided and bounded: that model may take the ticket unattended ([autonomy](autonomy.md)) |
| `process` | Changes the process itself |

A pull request says `Closes #<issue>` so that the merge closes the ticket.
Anything that must be versioned — the analysis, mock-ups, the owner's
decisions, acceptance criteria — is in `docs/tickets/<ID>.md` and the issue
links to it; the issue body is the summary, not a copy.

## Which model does what

| Stage | Model | Note |
| --- | --- | --- |
| Analysis that sets the bar: ticket analysis, mock-ups, estimates, decisions put to the owner, retrospectives, process changes | Fable | If Fable is unavailable, Opus may analyse and the ticket records it |
| Everything else by default: programming, tests, refactoring, QA reports, coordination, status, ordinary conversation | Opus 5.5 or newer | The working default |
| A ticket labelled `ready:sol`: acceptance tests, implementation, QA report, pull request | GPT 6.1 SOL (Codex) | Unattended, under [autonomy](autonomy.md); escalates instead of deciding |
| A ticket labelled `ready:sonnet`, and mechanical edits: a typo, a one-line change, formatting | Sonnet | Changes that contain no decision and no regression potential |

Fable is reserved for the work where the quality of the thinking decides the
outcome; a session that only coordinates, answers or implements runs on Opus.
The `ready:*` labels are given by the planning model when it closes the
analysis, never by the model that would take the ticket.

Subagents follow the same split: a planning agent delegates implementation to an
Opus subagent and trivia to a Sonnet subagent; an implementing agent never
delegates analysis to a smaller model. The pull request records which model did
the analysis and which did the implementation.

## What the repository contains

Tickets, decisions, evidence and the code. It does not contain the content of
conversations between the owner and an agent: a decision is recorded as the
decision and its reason, not as who said what. Nor does it point at them: a
commit made with an agent ends with `Co-Authored-By: Claude <model>
<noreply@anthropic.com>` and nothing more — no session trailer and no link to
a conversation, in commits or in pull requests.
