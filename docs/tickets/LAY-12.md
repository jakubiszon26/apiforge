# LAY-12: Acceptance of the LAY sprint — the owner's task on the room-booking scenario

- Size: S · Owner review: 10 min, plus the owner's own run · Risk: none
- State: rewritten on 2026-11-18 by the owner's decision (#296): the demo
  plan and the timed task sheet are replaced by one task a tester does on
  a scenario he has not seen. An agent ran it on `main` 2f6922c and the
  result set the minimal set below; the owner runs it on the build that
  holds that set · issue #296 · LAY sprint, last
- Analysis model: Fable · Implementation model: — (the owner at the window)

## Goal

The sprint is accepted when a tester, given an unfamiliar scenario and one
sentence, can find out from the window what a test really checks, follow a
value to its source, change that source without writing code, prove the
change, and read what was changed in the file — and can read dependencies
on a larger board too.

## The task

Project: `fixtures/scenarios/lay/rooms/` — a fictional room-booking API,
simulation only — copied fresh, no stored settings, window 1280 × 800.
Scenario **Double booking is refused**. The instruction, and nothing else:

> Check whether this test really detects a double booking of the same
> room. If you find a gap, fix the test and show how you know it now
> detects it.

Planted: the test is green; its second booking asks for **room B**
(`rooms.body.items[1].id`), which is closed for maintenance, and the test
asserts only `second.status == 409`. Variants for the wires:
**Double booking with a notice in between** and **Room booking day** (29
steps).

## Acceptance criteria

The owner judges each at the window; none is timed.

1. **What the test checks.** From the board: the order of the steps, that
   `first` and `second` are two bookings, and that the only check on
   `second` is its status.
2. **The source.** `second.body.roomId` is traced to its exact source —
   the producer step and the field, index included — and the place one
   started from is recovered. The same on *Room booking day*, where the
   source is out of view.
3. **The change.** That one field is made to read the room `first` booked,
   in the window, without typing code, knowing before it is applied that
   only this field changes.
4. **The proof.** From Run results the check `error.code ==
   "slot_conflict"` is added to `second`; the simulation fails
   (`room_unavailable`); with the sample of a real conflict (the control)
   it passes.
5. **The diff.** `git diff` of the `.afs` shows exactly the edits made:
   the one expression and the one new check, in its branch's braces, every
   other byte as written (`git diff` is the accepted way for 0.2.0, Q9 = A).
6. **The wires — a hard condition.** A finding is fixed in LAY when the
   tester loses a line, assigns data to the wrong source, takes a data
   line for order or order for data, or has to click several wires or pan
   back and forth to read one connection. A kink, a crossing or an
   asymmetry that still reads with certainty goes to LAY-2.
7. Every LAY issue is closed, or carried to LAY-2 with its reason in
   [lay.md section 13](../sprints/lay.md#13-the-close-2026-11-18).

## What the owner will see

Nothing new from this ticket. The build under test holds the minimal set
of [section 13](../sprints/lay.md#13-the-close-2026-11-18); each visible
change in it was accepted on a capture before its merge.

## Mock-up

Not applicable: no interface change.

## Not in scope

- New behaviour; fixing what the run finds (a new ticket, in LAY only
  under criterion 6).
- The demo plan of [lay.md section 10](../sprints/lay.md#10-demo-plan) and
  the timed task sheet of [section 9](../sprints/lay.md#9-metrics-and-qa):
  withdrawn as the acceptance of the sprint. The checkpoint's runs of the
  sheet stay recorded on #296.
- The LAY-01 table before and after: measured again after LANG-13
  ([section 13](../sprints/lay.md#13-the-close-2026-11-18)), when the
  reference scenarios change.

## Technical appendix

The agent-run trial (planning model, egui host, 1280 × 800, `main`
2f6922c) is summarised on #296: criteria 1 and 3 reachable with friction,
2 without a jump or a return on the larger board, 4 fully in the window,
5 right except that the new check was written outside its branch (#433).
Its findings W1–W9 are placed in section 13.
