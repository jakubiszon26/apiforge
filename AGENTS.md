# Agent guide

Coding agents and people follow the same rules in this repository.

- [Development process](docs/process/README.md): the lifecycle of a ticket,
  who decides what, the limits on size and work in progress, and which model
  does which kind of work. Read it first.
- [Sprints](https://github.com/jakubiszon26/apiforge/milestones): a sprint
  is a milestone; its description holds the goal, the exit criteria and the
  order of work, and its issues are the sprint's. Work starts from an open
  issue of the sprint with no `decision-needed` label; `gh issue list
  --milestone LANG` lists them. An agent told to pick a ticket and work
  follows [autonomy](docs/process/autonomy.md) and takes only a ticket
  labelled for its model. The [backlog](docs/backlog.md) keeps the record
  of closed sprints and the candidates for later.
- [Development guide](docs/development.md): toolchain, running, the verification
  every change passes before merge, and conventions.
- [GUI testing](docs/gui-testing.md): required for every change to the desktop
  interface, for reproducing interface defects and for design review.
- [Decisions](docs/decisions): architecture decisions; a change that alters one is
  recorded as a new or amended ADR.

## Rules an agent never bends

1. **Stop and ask** before removing or hiding anything visible, changing a
   default, touching backward compatibility or migration, or changing the
   project file format. These are the owner's decisions
   ([who decides](docs/process/README.md#who-decides)).
2. **No interface analysis without a mock-up** approved by the owner
   ([planning](docs/process/planning.md#mock-ups)).
3. **Acceptance tests before code**, in their own commit
   ([testing](docs/process/testing.md#1-acceptance-tests-first)).
4. **Branch from `main` only.** One ticket, one branch, one pull request
   from analysis to QA report; never a branch on an unmerged branch, and
   never a pull request that only updates a ticket
   ([planning](docs/process/planning.md#one-acceptable-result-one-pull-request)).
   A ticket's pull request does not edit `docs/backlog.md`. A change of plan
   alone goes to the milestone, or is committed to `main` directly by the
   owner or by the planning model in a session with the owner, and only in
   the files [planning](docs/process/planning.md#a-change-of-plan-needs-no-pull-request)
   names; an unattended agent never commits to `main`.
5. **At most four pull requests awaiting acceptance, within 60 estimated
   review minutes.** When either limit is reached, address review feedback
   instead of starting another.
6. **Only the owner merges to `main`**, after reading the QA report. The
   direct commits of a change of plan (rule 4) are not merges and carry no
   code.
7. **An ignored test names its ticket** in its reason, unless it is a
   design-review view or a spawned helper
   ([testing](docs/process/testing.md#1-acceptance-tests-first)).
8. **Delegate by the model policy**: analysis stays with the planning model
   (Fable); implementation goes to an Opus subagent (`model: "opus"`);
   mechanical edits to Sonnet. A ticket labelled `ready:sol` or
   `ready:sonnet` may be taken unattended by that model under
   [autonomy](docs/process/autonomy.md); the label is given by the planning
   model, never taken. An implementing agent does not delegate analysis
   downwards and does not decide what the owner decides: it escalates.
9. **The repository holds tickets, decisions and evidence**, never the content
   of conversations, and never a link to one: a commit ends with
   `Co-Authored-By: Claude <model> <noreply@anthropic.com>` and nothing else.
