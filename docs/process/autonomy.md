# Autonomy: a ticket another model takes unattended

The KTLO sprint sent every ticket, a one-line demo fix included, through the
planning model for its analysis and an Opus session for its code, while the
owner also runs GPT 6.1 SOL (Codex) and could hand it work. This document says
which tickets may be taken without the planning model in the loop, how an
agent picks one, what it must hand back, and who merges. It applies to any
model; the labels name the smallest model allowed.

## What makes a ticket ready

A ticket is labelled by the planning model when it closes the analysis, never
by the model that would take it. The label is a statement that every item
below holds; an agent that finds one does not hold stops and escalates.

| Label | Every item holds |
| --- | --- |
| `ready:sol` | The analysis is approved by the owner and the ticket file says so. Every owner decision the ticket needs is taken and recorded. The acceptance criteria are numbered and each names its test or its QA step. The mock-up, if the ticket has one, is approved as a whole. Size is `S` or `M`; regression potential is named and the test that covers it is listed. No `format`, `migration` or `removal` flag. |
| `ready:sonnet` | Everything above, and: size `S`, no risk flag at all, one file or one well-named unit touched, the change spelled out in the ticket to the level of what the code does, no judgement left to make. A typo, a wording, a constant, a test rename. |

A ticket with `decision-needed`, with an open question in its analysis, or
whose criteria say "probably" is never labelled.

## Picking a ticket

Told to pick a ticket and work, the agent:

1. Lists `gh issue list --label sprint:<current> --label ready:<model>` and
   takes the first by the backlog's order (the order of the table in
   [`docs/backlog.md`](../backlog.md), dependencies first); a ticket whose
   `after` ticket is not merged is skipped.
2. Checks the [work-in-progress limit](README.md#limits-that-keep-review-possible)
   against the open pull requests; if it is reached, it addresses review
   feedback on an open pull request of its own, or stops.
3. Branches from `main`, says in the issue that it has taken the ticket and
   with which model, and removes the `ready:*` label so no second agent takes
   it.
4. Works the ticket's own path: acceptance tests in their own commit, then
   the implementation, then the QA report, then the pull request with
   `Closes #<issue>`, the changed lines, and the models in the description
   ([testing](testing.md), [development guide](../development.md)). The
   `awaiting-acceptance` label goes on the issue.

## What it escalates

The agent stops, writes what it found in the issue, puts `decision-needed`
back on it, and does not continue the ticket, when:

- a stop condition of [planning](planning.md#stop-conditions) appears that the
  analysis did not record: something visible moves or disappears, a default
  changes, user data or the file format is touched;
- an acceptance criterion cannot be met as written, or meeting it needs a
  change outside the ticket's named files and crates;
- a test that is not the ticket's fails on `main` or in a way the ticket does
  not explain;
- the size proves wrong by a class, or the work needs a design the analysis
  did not make;
- anything would require guessing what the owner wants.

Escalation is not failure; a ticket that escalates has done its job of
measuring the analysis. The planning model answers in the issue and relabels
the ticket, or takes it back.

## Who merges

The owner merges to `main`, after reading the QA report. An agent merges only
when the owner has written an authorisation **in the issue of that ticket**
that names merge; a general instruction to "work autonomously" does not
include it. An agent never merges its own pull request; an authorised merge is
by an agent other than the one that wrote the change, and the merge commit
says so.

## What the planning model owes

For the path to work, the planning model closes every analysis it finishes
with the question "may this be labelled?", labels what qualifies, and writes
the ticket file to the standard the label promises. A ticket the agent had to
escalate for something the analysis should have said is recorded in the
retrospective against the analysis, not against the agent.
