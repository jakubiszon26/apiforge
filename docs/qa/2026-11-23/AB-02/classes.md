# AB-02: T6 on main after DIAG-06b, classed

Proposed by the planning model on 2026-11-24 from [results.md](results.md)
(pull request #595), read from the record only: `main` at `fe74ae73`
against v0.1.0's one recorded run of
[AB-01 round 4](../AB-01/round-4/results.md). It closes the point
[AB-01's classes](../AB-01/classes.md#not-decidable-reordering-a-step-t6)
left open. **Confirmed by the owner on 2026-11-24** (quiz; recorded on
#583 and #598), with #598 answered A
([regressions](../../../process/ab-testing.md#regressions)).

## Is T6 decidable now

**Yes, and it is better on `main`**, as a difference of kind, not of
count.

- v0.1.0 still has one run and no shortest path, so its spread is not
  known and no figure is compared as a figure. What is compared is what
  the runs had to do: on v0.1.0 a new request lands loose at the top of
  the board and is brought into the chain by four drags and two order
  wires, and a step is reordered in the *Execution order* lists (four
  changes, one refused as a cycle). On `main` the request is inserted in
  place by one menu choice and the move is one entry of the step's menu.
- *Pointer actions* 12 · 16 · 13 against 89 and *Wrong turns* 0 · 2 · 1
  against 14 are not a difference a second and a third run of v0.1.0
  could close. v0.1.0 opens on Code with the source on screen (R2, #548),
  which favours it; it came out worse all the same.
- AB-01's open question was whether a tester can move a step.
  **Move earlier was the first place every driver looked** (three counted
  runs and the void attempt) and one choice did the task. The move by
  copy, paste, delete and rename of v0.2.1-rc.2 (58 actions, 11 wrong
  turns) is gone.

## Worse than v0.1.0

Nothing. No class 1, no class 2: every run passed `check.sh` with the
two lines asked for, and no wrong turn was made in every first-time run.

## The wrong turns

| | Wrong turn | Runs | Class | Goes to |
| --- | --- | --- | --- | --- |
| W1 | The menu of `profile`, headed "After: profile · What should happen next?", adds the request last (step 30), bound to `profile`'s data | Run 2; the void attempt | **3** as a cost (one Undo, one of three counted runs). **A defect apart from its class**: the heading promises what *Add step* and the `+` do from the same place, and a tester who does not look at the end keeps a request that runs last | DIAG-22 [#598](https://github.com/jakubiszon26/apiforge/issues/598), new; decided A, `ready:opus` |
| W2 | *Output data* does not take the sample response | Run 2; the void attempt | None: **the same on both versions** (round 4) | REQ-15 [#576](https://github.com/jakubiszon26/apiforge/issues/576) |
| W3 | A right-click on the line `profile` → `team` opens the board's general *Add a step to the scenario* | Run 3; the void attempt | **None**: v0.1.0 has no insertion on a line, and it cost one Escape, after which the `+` showed and was used | No ticket ([below](#w3-the-right-click-on-a-line)) |

### W1: the cause

Not a regression against v0.1.0, which has the same menu and heading,
and not something DIAG-06b (#491) or DIAG-06a promised: neither ticket
names a step's own menu. It is a leftover. A right-click on a step opens
the menu from the step's first **data** output
(`canvas/mod.rs:2522`, `canvas/menu.rs:111`), the menu heads itself
"AFTER: …" for any source (`menu.rs:279`), and a block added from a data
output is bound to it and written last
(`apiforge-core/src/edit/nodes.rs:113`, `:131`); only *Next* writes it
after its step (`nodes.rs:120`). Since LANG-13 written last is run last.
Reproduced by a failing headless test on the branch
`test/repro-ab-02-insert-after` (`40dff6f2`): the new step is at place
17 of 18, expected 3. Where a step lands is a default, so the fix
waited for the owner: **A**, the step is inserted after the step it was
taken on, unbound, as the other two ways do, in the folder window.

### W3: the right-click on a line

DIAG-06b's `+` covers the insertion: it appears when the line is pointed
at, and both drivers that right-clicked the line used it next. The menu
that opens is the board's, by design of DIAG-06b (a right-click on the
`+` itself opens it too, `canvas/mod.rs:4241`); a step taken from it is
written last, as from the empty board. Two of four fresh drivers
right-clicked the line first, which is worth remembering and not worth a
ticket: if DIAG-22 is answered A, the same sentence, "the menu opened on
a place adds there", can be extended to the line in that ticket's review
for a few lines of code. Agents do not hover before they click; a person
sees the `+` first.

## The void ruling

**It stands.** The first attempt at run 2 sent six pairs of actions
without a look between them. By the letter of the
[process](../../../process/ab-testing.md#how-the-agent-drives-the-window)
blind input is a driver error, counted apart, and only a broken
prohibition voids a run; the brief of round 4 is stricter and the
coordinator followed the brief. That is the right way round here: six
actions spread over the whole run were sent before the result of the one
before was seen, so what the driver expected at each cannot be told from
what it met, and that is what a wrong turn is counted by. The ruling was made on the driver's own report, not on its result
(it passed), and the repeat was a fresh driver on a fresh copy.

It also changes nothing: the void attempt's figures (16 pointer actions,
3 wrong turns) lie beside run 2's (16 and 2), *Move earlier* was found
the same way, and its three wrong turns are W1, W2 and W3. They are used
above only as a second sighting of a place, never as a figure. For the
runbook: the process text should say what the brief says, that a run
with input sent unseen is void (one line, with KTLO-48 #581).

## Better than v0.1.0 and v0.2.1-rc.2

- A step is moved from its own menu, found at once (DIAG-06b).
- A request is inserted in place: *Add step* with a step selected
  (DIAG-06a) or the `+` on the line (DIAG-06b); the first-time runs used
  each.
- The file gets the lines asked for; the shortest path and run 3 left
  byte-identical files.
- A fix of W1 must keep all three.

## What this does not tell

- Alt+↑/↓, the refusal messages and a move over more than one place were
  not exercised: the task needs one move by one.
- Moving a step between Main and Cleanup: no command does it (REQ-14
  [#549](https://github.com/jakubiszon26/apiforge/issues/549)); T6 does
  not ask for it, whatever #583's title says.
- Whether DIAG-21 (#580, a card dropped on a line) is still wanted: no
  driver tried a drag once the menu had *Move earlier*. Fable's proposal:
  answer C on #580 (neither) until a task needs a move over many places.

## The owner's answers (2026-11-24)

1. Confirmed: T6 decided, better on `main`; nothing worse than v0.1.0;
   W1 class 3 and a defect (#598), W2 the same on both, W3 no ticket; the
   void ruling stands.
2. #598: **A**.
