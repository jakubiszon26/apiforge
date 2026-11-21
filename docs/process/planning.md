# Planning and analysis

Analysis exists so that the owner can say "that is not what I meant" before any
code is written. Stage II failed this test: a 437-line specification described
transactions, sessions and recovery in detail and never listed the screens, so
the block palette was missing from the default mode and nobody noticed until
the stage had shipped.

## The ticket

A ticket is a GitHub issue titled `<ID>: <goal>`, labelled as the
[process](README.md#labels) says, created with `gh issue create` when the
ticket is accepted into a sprint. When the issue is not enough — anything
above `S`, or anything with a risk flag — the ticket also gets a file
`docs/tickets/<ID>.md`, linked from the issue, with these sections, in this
order:

```markdown
# KTLO-05a: Block palette in the folder mode

- Size: S · Owner review: 20 min · Risk: visible UI
- State: Analysis
- Analysis model: Fable · Implementation model: —

## Goal
One or two sentences in the owner's words. What the person using the
application will be able to do afterwards.

## What the owner will see
- New: …
- Changed: …
- Removed or hidden: …          ← triggers an owner decision
- Migrated or converted: …      ← triggers an owner decision
- Default behaviour that changes: …  ← triggers an owner decision

## Mock-up
Path of the capture(s), or "not applicable: no interface change".

## Acceptance criteria
Numbered, each one testable by a person at the window or by a test.
1. …
2. …

## Not in scope
What this ticket deliberately leaves out, and which ticket takes it.

## Proposals beyond the request
The planning model's [own proposals](#the-planning-models-own-proposals),
one row each: problem solved · size and risk · trade-offs · where it would
go. Or "none". A ticket of size S may leave the section out.

## Technical appendix (optional)
Crates and files touched, reuse of existing code, open technical questions.
The owner is not expected to read this section.
```

The sections above the appendix fit on one page. If they do not, the ticket is
too large: split it.

## One acceptable result, one pull request

A ticket delivers one result the owner can accept, and that result travels as
**one pull request**, opened as a draft when the analysis starts:

1. The first commit is the analysis: the ticket file, the mock-up captures,
   and the decision card posted to the issue.
2. The owner's decisions are recorded in the ticket file in a commit of the
   same branch. The pull request stays a draft until the analysis is approved;
   no implementation commit before that.
3. The acceptance tests follow in their own commit, then the implementation,
   then the QA report; the draft becomes ready for review.

What this rules out: a pull request whose only content is a backlog row, a
recorded decision or a sentence of a ticket. Those changes ride on the pull
request of the ticket they describe, or on the next pull request that touches
the backlog. A separate analysis pull request is right only when the analysis
unblocks several tickets (KTLO-05a, LAY-00, MCP-00 are the pattern); a
one-ticket analysis is the first commit of the ticket's own pull request.

An analysis that stops at a decision the owner has not yet taken leaves the
draft open with the `decision-needed` label on the issue; the branch waits,
nothing else is started on it.

## A rule several tickets share has one owner

When several tickets depend on one rule — what a conversion may write
and under which reading, where a step may be placed, what a deletion
takes with it — the rule is written in **one** ticket, which the others
name and call; no second ticket restates it in its own words. That
ticket is analysed and reviewed first, by the model that writes it and
by a second one ([autonomy](autonomy.md#what-makes-a-ticket-ready)),
before the tickets around it are written. Introduced on 2026-11-20: the
LANG tickets for updating a project each described the same plan and
commit, differently; three rounds of review found the defects on the
seams between them, not inside any one.

## Mock-ups

An analysis that changes the interface includes a mock-up before approval. A
mock-up answers "what will it look like at 960 × 540 and at 1600 × 1000", not
"how will it work". A static capture does not show a gesture: when the change
has an interaction (dragging, a mode that persists, a state shown while
typing), the acceptance criteria name the gesture and the state, not only the
control. KTLO-18 and KTLO-19 approved a button and shipped a panel whose edge
still would not drag; the owner found it after acceptance.

Acceptable forms, in order of preference:

1. **A design review capture** from a throwaway branch: a `review_*` test (see
   [GUI testing](../gui-testing.md#design-review)) that renders the intended
   layout with static or placeholder content, captured by
   `tools/gui/review.sh <filter>`. The branch is deleted after the decision.
   The captures the owner decided on are committed with the ticket under
   `docs/qa/<date>/<ticket>/`, as the evidence of that decision; captures of
   rejected rounds are not.
2. **An annotated capture of the current view** when the change is small
   enough to describe by marking an existing screenshot.
3. **A labelled wireframe** (text or drawing) only for a screen that does not
   exist yet in any form, and only as the first round before form 1.

**A comparison board** is how a choice between options is shown (the owner,
2026-11-21, after the 0.2.1 mock-ups): one image per question with today's
view and every option side by side, each panel captioned (*Today (v0.2.0)*,
*A · …*, *B · …*), cropped to the element in question at the same scale in
every panel so the difference is visible at a glance. The full-window
captures at 960 × 540 and 1600 × 1000 stay beside it, as the check of how the
recommended option sits in the whole window; the board does not replace
them. Examples: `docs/qa/2026-11-21/UX-41/selection-failed-100.png`,
`docs/qa/2026-11-21/UX-42/run-mode-live-960.png` on `mock/rel-0.2.1`.

A board shows how options look, not what a task costs on each. When the
question is about a gesture or a flow, the options can also be compared by
agents doing one task in the real window on each of them: an
[A/B round](ab-testing.md), for the tickets a sprint's plan names. Its table stands beside
the board in the same quiz; it does not replace the board or the owner's
choice.

A mock-up is approved as a whole or sent back; approval is noted in the
ticket's state. The implementation is then measured against the mock-up in the
QA report.

The owner looks at mock-ups in the issue, not in the repository tree. Once
the captures are committed on the ticket's branch, post them to the issue as
images by their immutable raw URL, one per view with its caption:

```markdown
![Diagram at 960 × 540](https://raw.githubusercontent.com/jakubiszon26/apiforge/<commit>/docs/qa/<date>/<ID>/<view>.png)
```

`<commit>` is the commit that added them, so the link survives the branch
being deleted after merge. The same applies to the captures of a QA report
when the pull request asks for a decision.

## Stop conditions

The agent stops analysis and asks the owner when any of these appears:

- The change removes, hides or moves a feature or a screen the owner can see.
- The change alters what happens by default: which window opens, what a key
  does, where a panel sits, what a new project contains.
- The change touches backward compatibility or migrates user data.
- The change alters the project file format or anything Git tracks in a
  project.
- Two reasonable designs exist and they look different to the owner.

The question is one paragraph: the decision, the recommendation, the
consequence of each option for the person at the window. Technical reasoning
goes below the fold.

## The planning model's own proposals

The owner is not the only source of features. An analysis answers what was
raised; it also asks what the person was trying to get done and whether
something else would get it done better. The analysis of the owner's run
of 2026-11-19 placed ten findings about views and proposed nothing of its
own, while the scenario in front of it repeated one request sixteen times:
a table of cases would save more than any view, and the owner had to name
it.

An analysis of a sprint, of the owner's findings or of a ticket above `S`
therefore ends with a section of the planning model's proposals beyond the
points raised — a feature, an optimisation, or a simplification that
removes the need for the thing asked. Each says in a row:

- the problem it solves, in terms of the owner's time or mistakes;
- its size and risk flags, by [estimation](estimation.md);
- its trade-offs, including what it makes unnecessary and what it costs
  elsewhere;
- where it would go: the current sprint, a named later one, or "later".

The strongest become candidate issues, labelled `decision-needed`, whose
status line says they are proposed and not decided. A proposal is never
scheduled, labelled `ready:*` or built without the owner's decision, and it
does not widen the ticket under analysis. An analysis that finds nothing
worth proposing says so in one line; a one-ticket analysis of size `S` may
leave the section out.

## Analysis checklist

Before asking for approval:

- [ ] Every screen, panel, menu and dialog the ticket touches is named, and
      the ones that exist today were looked at in the running application or a
      capture, not inferred from code.
- [ ] Existing code that already does part of the job is named in the
      appendix (search before designing: `rg`, the ADRs, `docs/`).
- [ ] The acceptance criteria can each become one test or one QA step.
- [ ] The estimate follows [estimation](estimation.md) and an `L` is split.
- [ ] Every stop condition was checked and the owner's decisions are recorded
      in the ticket.
- [ ] The planning model's [own proposals](#the-planning-models-own-proposals)
      are in the analysis, or it says there are none, and the strongest are
      candidate issues (a one-ticket analysis of size `S` may leave them
      out).
- [ ] The analysis was written by the model the [process](README.md#which-model-does-what)
      assigns to it.
