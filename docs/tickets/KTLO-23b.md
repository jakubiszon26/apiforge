# KTLO-23b: A component runs from its tab — the Run inputs dialog

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: Approved 2026-10-27 (mock-up); after KTLO-23a
- Analysis and acceptance criteria: `docs/tickets/KTLO-23.md`
- Analysis model: Fable · Implementation model: —
- After KTLO-23a

## Delivery

A *Run inputs* dialog before a component run: one field per input with its
type, pre-filled with the input's default, values kept for the session per
component. With decision 1 = A it appears only when an input has no default;
with B before every component run. As mocked and approved, the dialog
pre-fills the defaults (`quantity` = 1 in the mock-up) and keeps *Run*
disabled until every input has a value.

Criteria covered: KTLO-23 criterion 9, as an interaction test through the
named dialog widgets, plus the dialog's view in the mock-up.

## Not in scope

Remembering values across sessions or in the project; typed editors beyond
text, number, boolean and literal JSON.
