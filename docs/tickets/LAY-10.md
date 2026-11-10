# LAY-10: Focus on selection — the selection state and the relation model

- Size: S–M · Owner review: 20 min · Risk: `visible UI`, `default`
- State: Backlog (issue #294) · decided by the owner on 2026-11-05 (Q9 = A)
  · LAY sprint, part B, first after the checkpoint
  ([review of 2026-11-10](../sprints/lay.md#11-mid-sprint-review-2026-11-10));
  after LAY-22 and after #288 is fixed (selecting a step must not mark the
  document changed, or focus would stale the results it reads)
- Analysis model: Fable · Implementation model: Opus

## Goal

Selecting a step answers "what feeds this, what reads it, what checks it"
without a mode: the direct producers, consumers and checks ring, the rest
dims, and Esc restores. The selection state and the relation model this
introduces (producers, readers, checks, gates) are reused by LAY-14, LAY-15
and LAY-11.

## What the owner will see

- New: selecting a request rings its direct producers and consumers with a
  lighter outline in another hue and ringed ports, and its checks in the
  check colour; unrelated cards dim to about 60 % (names and statuses stay
  legible) and unrelated wires to about 25 %. Selecting a check rings its
  subjects and value sources. Esc clears the selection and the dimming.
- Four signals are told apart without reading the footer (P4): the
  selection (strong accent frame and handles), the related cards (lighter
  outline, another hue), the running step (the glyph in its badge, LAY-07),
  a failure (red mark and status). A failed or running card is never dimmed.
- Changed: the look of the board while something is selected. Removed or
  hidden: nothing. Default behaviour that changes: the dimming is on by
  default; there is no toggle (Q9 = A).

## Decisions taken (owner, 2026-11-05)

> Q9 = A: focus is selection behaviour only; direct producers, consumers
> and checks ring, the rest dims, Esc clears. A toolbar *Focus* toggle for
> the full chain (option B) was not chosen.

The planning model's parameters, for veto on the QA captures: cards dim to
60 %, wires to 25 %; the related outline's hue is not the selection's accent
(4.4 item 6).

## Mock-up

Round 3 `lay00r3-focus-offscreen-60`, round 4 `lay00r4-run-focus-60`
(`mock/lay-00-round-3` at `ea981d8`, `mock/lay-00-round-4` at `df6305b`).
Approved as direction on 2026-11-05.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/lay10.rs`, on `shop-30.afs` after a failed simulation.

1. Selecting `cart` rings `login`, `first`, `second`, `cart_total` and
   `order` with the halo and their ports; every other card is dimmed; the
   selection keeps its handles. Test:
   `lay10_criterion_1_selecting_cart_rings_its_relations`.
2. A failed card and the running card are never dimmed, whatever is
   selected. Test: `lay10_criterion_2_failed_and_running_are_never_dimmed`.
3. On a dimmed card the name's contrast against the card stays at least
   3:1 (the dimming is an alpha over the paint, not over the text). Test:
   `lay10_criterion_3_names_stay_legible_when_dimmed`.
4. Esc restores every card and wire to full strength and clears the
   selection; clicking the empty board does the same. Test:
   `lay10_criterion_4_esc_restores`.
5. The rings and the dimming read LAY-22's `relations`; this ticket adds
   no relation of its own.
6. Selecting a check rings its subjects and value sources in the check
   colour. Test: `lay10_criterion_6_a_check_rings_its_subjects`.

## Not in scope

Tracing a single wire or input (LAY-14); relations out of view (LAY-15);
a *Focus* toggle for the full chain (option B, not chosen); the collapse.

## Technical appendix

- `canvas/mod.rs` gains a `Selection` with the selected step and the
  `Relations` of `apiforge-core` computed on selection change; `paint::card`
  and `paint::routed_wire` take an alpha — a second pass over the paint, no
  geometry change (the plan's appendix).
- UX-31's ring on the reading steps is the same ring; LAY-10 generalises it
  (interplay named in the plan, section 7).
