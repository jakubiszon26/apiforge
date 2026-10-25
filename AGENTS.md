# Agent guide

Coding agents and people follow the same rules in this repository.

- [Development process](docs/process/README.md): the lifecycle of a ticket,
  who decides what, the limits on size and work in progress, and which model
  does which kind of work. Read it first.
- [Backlog](docs/backlog.md): the board. Work starts from a ticket in `Ready`.
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
4. **Branch from `main` only.** One ticket, one branch, one pull request,
   within its size class; never a branch on an unmerged branch.
5. **At most three pull requests awaiting acceptance.** When the limit is
   reached, address review feedback instead of starting a fourth.
6. **Only the owner merges to `main`**, after reading the QA report.
7. **An ignored test names its ticket** in its reason.
8. **Delegate by the model policy**: analysis stays with the planning model
   (Fable); implementation goes to an Opus subagent (`model: "opus"`);
   mechanical edits to Sonnet. An implementing agent does not delegate
   analysis downwards.
9. **The repository holds tickets, decisions and evidence**, never the content
   of conversations.
