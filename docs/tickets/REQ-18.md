# REQ-18: A request is saved as a component in one action

- Size: S–M · Owner review: 15 min · Risk: `visible UI`, `default` (what
  the dialog ticks)
- State: **Analysis; four questions wait for the owner.** From the
  owner's wish of 2026-11-24: "It would be nice if such a request could
  be saved as a component straight away." Not scheduled, no `ready:*`
  label · issue [#604](https://github.com/jakubiszon26/apiforge/issues/604)
  · REQ sprint · after REQ-02a; from a try, after
  [REQ-17](REQ-17.md)
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A request that works — in a scenario or as a try — becomes a component
with inputs and an output, ready to call, without building the
component by hand.

## Today

*Save as component* exists (UX-30, `folder/save_component.rs`), and for
one request it already does most of the wish: on the board, select the
request → *Selection actions* → *Save as component* → a dialog that
lists *URL*, each header and **each field of the body** as an input,
unticked, and *Response* as the output, ticked → *Save and publish*;
revision 1 is published and called in place of the step
([today's dialog](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/board-component-960.png),
left panel). What is missing:

- It is offered on *Canvas* only; *Steps*, the request pane and a try
  have no selection actions.
- Nothing is ticked and a row does not show its value: the person
  decides by name alone which values vary.
- A proposed name is the slot's path (`body_productId`), retyped by
  hand.
- The address is one input, whole: a segment or a query value inside
  it cannot be chosen.

Counted from the code and the rendered dialog, for a component with two
inputs: **6 pointer actions and 3 text entries** today (select,
*Selection actions*, *Save as component*, two ticks, *Save and
publish*; the name and two input names); one more after the decided REQ
tickets (the switch to *Canvas*); **2 pointer actions and at most 1
text entry** under A.

## What the owner will see

- New: *Save as component…* in the request pane's menu, a card's menu in
  *Steps* and a try's *Keep ▾*; in UX-30's dialog, each row with its value, the
  parts of the address as rows, short names, and ticks suggested.
- Default behaviour that changes: which inputs the dialog ticks for a
  single request (question 1). The board's dialog for several steps is
  unchanged.
- Removed, hidden, migrated: nothing. **The format is not changed**: the
  component's text is what adding the inputs and typing the
  placeholders writes today.

## Mock-up

Branch `mock/req-17`, commit `df0b06c1`.
[Board, 960 × 540](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/board-component-960.png) ·
[A at 960](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/window-req18-a-960.png) ·
[A at 1600](https://raw.githubusercontent.com/jakubiszon26/apiforge/df0b06c18e689eb1f05d8c24bcd6c0188d0ced34/docs/qa/2026-11-24/REQ-17/window-req18-a-1600.png).
The dialog of A is drawn compact; the built one is UX-30's own with its
rows.

## Questions for the owner

Recommended first.

1. **Which values are inputs.** Listed always: each field of a literal
   body, each segment of the address after the host, each query value,
   each header value except `Content-Type`. Never offered with its value
   as a default: `Authorization` (it is offered as an environment
   secret, the rule of REQ-07). **A — ticked: the values the person
   changed between sends in this window; none when nothing was
   changed.** · B — ticked: every body field and every path segment that
   is a number or an id. · C — nothing ticked, as today.
2. **A dialog, or one action.** **A — UX-30's dialog, opened with the
   name taken from the request.** · B — no dialog: the component is
   written with question 1's inputs and a line offers *Edit inputs* and
   *Undo*.
3. **The output.** **A — the response body, as today; *HTTP status*
   offered unticked.** · B — body and status. · C — also fields picked
   from the last response.
4. **Revision and the step.** **A — as COMP-00 decided (5): *Publish
   revision 1* is the default and *Only create the file* the other
   choice; a step of a scenario is replaced by the call and its checks
   read the call's output; a try stays open as it was.** · B — a draft
   by default. · C — the step is always kept.

## Acceptance criteria

For A in each.

1. *Save as component…* is offered in the three places named and opens
   the same dialog as the board's for that one request.
2. The dialog lists the values of question 1 with where each comes from
   and its value; a ticked one becomes an input whose default is that
   value and is optional (KTLO-36).
3. The component's text passes the check and equals what the hand path
   writes; called with no inputs it sends the request as it was.
4. A value changed between two sends of this request in this window is
   ticked and says so; a new window ticks none.
5. `Authorization` is never written as a default; `Content-Type` is not
   offered.
6. From a scenario the result is UX-30's (one undoable edit; the checks
   after the step read the call); from a try the try is unchanged and
   the component is in *Project components*.
7. At 960 × 540 the dialog's rows are not cut; a long value is shortened
   with its full text on hover.
8. QA step: part 3 of REQ-06, its last sentence.

## Not in scope

Several requests at once from *Steps* (the board's selection does it);
updating a call to a newer revision (COMP-01); a component made from a
cURL command (REQ-07 then this).

## Technical appendix

- Reuse: `SaveComponent` and `edit::propose_extraction`;
  `initialize_inputs` (`apiforge-core/src/edit/component.rs:150`) gives
  the leaves of the body and the headers already. New: the literal
  parts of the address template as leaves, a ticked one rewritten to
  `param.<name>`; the last path segment as the proposed name; the ticks.
- Regression: the board's dialog for several steps (UX-30's and UX-37's
  tests unchanged); KTLO-32 and KTLO-36 on required inputs.
