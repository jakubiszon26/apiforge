# Retrospective

A retrospective closes every sprint. It is written by the planning model with
the owner, from the repository's own numbers, and ends in at most three
actions, each of which is a ticket. It lives in
`docs/retrospectives/<yyyy-mm-dd>-<sprint>.md`.

## Facts first

Collected before any opinion, from Git and the backlog:

| Measure | Source |
| --- | --- |
| Pull requests merged; median and maximum changed lines | `git log --merges`, `git diff --shortstat` |
| Commits by type (`feat`, `fix`, `test`, `docs`) | `git log --format=%s` |
| Estimate vs actual per ticket: size, review minutes, rounds of changes | Ticket files |
| Tickets that changed class or were split mid-sprint | Backlog history |
| Defects found **after** owner acceptance | Issues or backlog rows opened against merged tickets |
| Ignored tests, with and without a ticket | `rg '#\[ignore'` |
| Owner decisions asked for, and how many were asked late (after code existed) | Ticket files |
| Smoke list runs per platform | QA reports |

The target that matters most is **defects found after owner acceptance: 0**.
A fix/feat ratio near one inside the sprint means the acceptance tests did not
describe the criteria.

## Three questions

Answered briefly, with reference to the facts:

1. What worked and should stay.
2. What did not work, and which rule or missing rule allowed it.
3. What surprised us: an estimate off by a class, a decision asked late, a
   flow nobody checked.

## Three actions

At most three. Each action is a ticket in the backlog with an owner, and each
changes a rule, a template, a list or a tool rather than asking people to be
more careful. An action that cannot be expressed as a change to
`docs/process/`, a template or a script is not an action.

## Template

```markdown
# Retrospective: <sprint> (<dates>)

## Facts
| Measure | Value |
| --- | --- |
| … | … |

## What worked
- …

## What did not
- … — allowed by: <rule or gap>

## What surprised us
- …

## Actions
| # | Action | Changes | Ticket |
| --- | --- | --- | --- |
| 1 | … | docs/process/testing.md | KTLO-12 |
```

The retrospective records decisions and reasons, never the content of
conversations.
