# REQ-18: A request is saved as a component in one action

- Size: M · Owner review: 15 min · Risk: `visible UI`, `default` (what
  the dialog ticks)
- State: **Decided** by the owner on 2026-11-24 ([#604](https://github.com/jakubiszon26/apiforge/issues/604)): 1 A,
  2 A, **3 B**, 4 A. Mock-up approved (panel A; the built dialog is
  UX-30's own, not the compact drawing). After REQ-02a and REQ-05;
  from a try through REQ-17d.
- Analysis model: Fable 5.2 · Implementation model: Opus

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
text entry** as decided.

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

## Decisions (owner, 2026-11-24)

| | Decided | Not chosen |
| --- | --- | --- |
| 1 = A | Ticked: the values the person changed between sends in this window; none when nothing changed | every body field and id; none ever |
| 2 = A | UX-30's dialog, improved, from a try, *Steps* and Canvas | one action with no dialog |
| 3 = B | Outputs: the response **body and the status, both ticked** | body only; fields picked from the response |
| 4 = A | Publishes revision 1 by default; a scenario's step is replaced by the call; a try stays open | a draft by default; the step always kept |

## Acceptance criteria

Tests in `folder/acceptance_tests/req18.rs`; unit tests in
`apiforge-core/src/edit/component.rs`.

1. *Save as component…* is in the request pane's menu and the card's
   menu in *Steps*, and stays in the board's *Selection actions*; each
   opens the one dialog for that request. `req18_criterion_1_*`
2. For one request the rows are: each field of a literal body, each
   header except `Content-Type`, each query value, and each segment of
   the address after the host. A row shows where the value is and the
   value. The board's dialog for several steps lists what it lists
   today. `req18_criterion_2_*`
3. A row's proposed name is its last path segment (`productId`, not
   `body_productId`), made unique by a number. `req18_criterion_3_*`
4. A value that differed between two sends of this request in this
   window (a run, *Send alone* or a try's *Send*) is ticked and says in
   how many; with no such value nothing is ticked; a new window ticks
   nothing. `req18_criterion_4_*`
5. A ticked literal becomes an input whose default is that value and is
   optional (KTLO-36); a ticked segment of the address rewrites that
   part to `param.<name>`; called with no inputs the component sends
   the request as it was. `req18_criterion_5_*`
6. A value under a sensitive name (`is_sensitive_key`) is listed
   without its value, never ticked by criterion 4, and when ticked by
   the person becomes a required input with no default.
   `req18_criterion_6_*`
7. **Outputs: *Response* (body) and *HTTP status* are both ticked** for
   one request, with whatever a later step reads; headers and duration
   offered unticked. For several steps the outputs are as today.
   `req18_criterion_7_*`
8. *Publish revision 1…* is the default. From a scenario the result is
   UX-30's: one undoable edit, the step replaced by the call, the
   checks after it reading the call's `body` and `status`.
   `req18_criterion_8_*`
9. The component's text passes the check and is what adding those
   inputs and typing the placeholders writes: no format change.
   `req18_criterion_9_*`
10. At 960 × 540 the dialog's rows are not cut: a long value is
    shortened with its full text on hover. `assert_within_window`
11. QA step: part 3 of REQ-06, its last sentence.

## Captures the owner judges on the pull request

The dialog for one request with two ticked rows, at 960 × 540 and
1600 × 1000; a row of the address; a row under a sensitive name; the
scenario after the call replaced the step.

## Not in scope

Several requests at once from *Steps* (the board's selection does it);
updating a call to a newer revision (COMP-01); a component made from a
cURL command (REQ-07 then this); the entry in a try's *Keep ▾*
(REQ-17d, criterion 6, built after this ticket).

## Technical appendix

- Reuse: `SaveComponent` and `edit::propose_extraction`;
  `initialize_inputs` (`apiforge-core/src/edit/component.rs:150`) gives
  the leaves of the body and the headers already. New: the literal
  parts of the address template as leaves, a ticked one rewritten to
  `param.<name>`; the last path segment as the proposed name; the ticks.
- Files: `apiforge-core/src/edit/component.rs`, `edit/extract.rs`;
  `folder/save_component.rs`; new `steps/changed.rs` (what differed
  between sends, fed where `folder/runs.rs` takes a finished run or
  single send; memory only); `steps/request.rs`, `steps/list_edit.rs`
  (the entries); the test file.
- Regression: the board's dialog for several steps — UX-30's and
  UX-37's tests unchanged, and criterion 2's last sentence; KTLO-32 and
  KTLO-36 on required inputs; the default of the outputs (criterion 7)
  is new for one request only.
