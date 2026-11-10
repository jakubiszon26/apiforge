# COMP-00: Component authoring in Diagram, the mock-up and its decisions

- Size: S (mock-up) · Risk: `visible UI`, `format` (resolved by decision 1:
  nothing is stored) · issue #239
- State: decided by the owner on 2026-11-02 in a quiz; the mock-up is
  **approved as a whole**
- Mock-up: throwaway branch `mock/comp-00` at `5ea3b9c`; captures under
  `docs/qa/2026-11-02/COMP-00/` on that commit. The captures of the chosen
  strip (Q1 = C) are copied here with UX-31.
- Source issues: #174 (confirmed in real work on 2026-11-02: a component's
  inputs were hard or impossible to find and edit graphically), #161 part 2,
  #172 parts 1–2, #173, #170 part 1, #165

## Decisions

1. **Inputs on the component board: C, an INPUTS strip** under the board's
   header row: one pill per input and *+ Add input*, no wires. Selecting a
   pill rings the steps that read it. The strip stores nothing, so the
   project format is unchanged; the follow-ups on where to store option A's
   card or option B's chips in `@layout` do not apply.
2. **+ Add input: A**, a *New input* form in the Inspector.
3. **Editing an input in the Inspector**: approved as captured, plus live
   edits: each change shows on the board as it is typed (#172 part 2). An
   invalid name stays in the field with an inline error, and the code keeps
   the last valid name.
4. **Component settings**: a *Component settings* button in the board
   header opens them (the owner's choice, not the recommendation). Not
   rendered; judged on the implementation's QA captures.
5. **Save as component** (UX-30): the dialog offers both outcomes, with
   *publish revision 1 and call it here, in place of the selected steps* as
   the default; the other choice only creates the file.
6. **Publish with nothing to publish: B**, a disabled *Publish* with
   *r1 · no changes* beside it.
7. **Copy, paste, duplicate** (#173): not rendered; its own ticket after
   COMP-00.

## Tickets

| Decision | Ticket | Issue |
| --- | --- | --- |
| 1 | UX-31: the INPUTS strip, the ring, *+ Add input* | #248 |
| 2, 3 | UX-32: inputs added and edited in the Inspector, live | #249 |
| 4 | UX-33: the *Component settings* header button | #250 |
| 5 | UX-30: *Save as component* | #238 |
| 6 | UX-25: *Publish* B | #233 |
| 7 | UX-34: copy, paste, duplicate (#173) | #251 |

## Captures (Q1 = C)

- [Diagram, 1600 × 1000](../qa/2026-11-02/COMP-00/comp00-q1-c-band-diagram-1600x1000.png),
  [960 × 540](../qa/2026-11-02/COMP-00/comp00-q1-c-band-diagram-960x540.png)
- [A pill selected, 1600 × 1000](../qa/2026-11-02/COMP-00/comp00-q1-c-band-selected-diagram-1600x1000.png),
  [960 × 540](../qa/2026-11-02/COMP-00/comp00-q1-c-band-selected-diagram-960x540.png)
- [Split, 1600 × 1000](../qa/2026-11-02/COMP-00/comp00-q1-c-band-split-1600x1000.png),
  [960 × 540](../qa/2026-11-02/COMP-00/comp00-q1-c-band-split-960x540.png)
