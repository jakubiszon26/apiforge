# REQ-13: The window writes only what was set

- Size: S–M · Owner review: 15 min · Risk: `default` (what a new step
  contains)
- State: **Analysis; three questions wait for the owner** · proposed by
  the planning model on 2026-11-21 as gap G7 of the mandatory scenario
  and accepted by the owner the same day as a ticket of its own · issue
  [#540](https://github.com/jakubiszon26/apiforge/issues/540) · REQ sprint
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A test built in the window is saved as the tester built it. Today three
requests and one check
([the file](../qa/2026-11-21/REQ-06/baseline-v0.2.0-cart-lifecycle.afs),
v0.2.0) come out with `@config({ method: "POST", timeoutMs: 10000.0 })`
above the POST, `timeout 10s` in each request and `@label("Check id")`
above the check. The diff of a test is longer than the test, a reviewer
cannot tell a timeout someone chose from one the window filled in, and
criterion 5 of [REQ-06](REQ-06.md#acceptance-criteria) cannot be met.

## What the owner will see

- Changed: a request added in the window and left at its defaults is
  written as its method, its address and what was typed into it.
- Default behaviour that changes: what a new step contains in the file —
  questions 1 and 2.
- Migrated or converted: nothing unless question 3 says so.
- Removed or hidden: nothing in the window.

## Mock-up

Not applicable: no interface change. The analysis shows the file of the
mandatory scenario before and after, side by side.

## Questions for the owner

Recommended first.

1. **A timeout equal to the default.**
   A. Not written; the line appears when the value is changed.
   B. Written as today — every request says its timeout.
2. **The label of a check made by one click** (`@label("Check id")` over
   `check_id`).
   A. None: the handle alone, as a check written by hand.
   B. Kept — the card shows *Check id* instead of `check_id`.
3. **Files saved by 0.2.0.**
   A. Left as they are; a line goes only when its step is edited in a way
   that rewrites it.
   B. `apiforge fix` offers to remove what equals the default, with its
   diff.
   C. Left as they are, always.

## Acceptance criteria

Written after the answers. Fixed already:

1. The mandatory scenario of REQ-06, built in the window, is saved with
   no `@config` line and no line for a setting left at its default.
2. A setting the person changed is written, and one changed back to the
   default follows question 1.
3. A file that already has such lines reads, runs and prints as today.
4. What the language accepts does not change.

## Not in scope

- `begin = start`: whether a scenario names its start is the language's
  (LANG), not this ticket's.
- How the formatter breaks a body (EDIT-03 #369); the demo project's
  text (DEMO-01 #523).

## Technical appendix

To find in the analysis: why a POST added in the window gets `@config`
while a GET does not (`apiforge-script`, `lower.rs` reads `@config` as
the whole configuration and refuses one that disagrees with the
statement); where the Inspector's defaults for a new request are set;
whether the printer can tell "equal to the default" from "set to the
default".
