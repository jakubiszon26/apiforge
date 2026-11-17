# LAY-30: Checks are expanded by default; their switch sits on the board's bar and on each step

- Size: M · Owner review: 25 min · Risk: `default`, `visible UI`
- State: Analysis. Mock-up posted to issue #383 on 2026-11-17; four
  questions wait for the owner. LAY sprint, cards track, after LAY-31
  (#384) and before LAY-35 (#410).
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A person who opens a scenario sees what it asserts: the check cards are
drawn. One visible control on the board's bar folds them all and shows them
again, and each step can fold or show its own.

## Decided by the owner (2026-11-16)

- Checks are **expanded by default**; this reverses decision 4 of
  [lay.md 5.1](../sprints/lay.md#51-decisions-taken-2026-11-05).
- The switch between expanded and collapsed **sits on the board's bar**, not
  in the view menu. Collapsed keeps the checks line in the step's footer
  (LAY-11).
- Beside the board's switch there is one **per step**, in both directions.
- A step's own choice is **remembered per document** in `.apiforge/local/`:
  this computer only, outside Git, no format change.
- The sprint's exit criterion "fits at 50 % or more" is read with checks
  collapsed; the expanded figure is reported beside it.

## Mock-up

Rendered by the throwaway branch `mock/lay-30-checks-switch`
(`review_lay30_mock`, `APIFORGE_LAY30=a|b`), `shop-30` tidied, in
[`docs/qa/2026-11-17/LAY-30/`](../qa/2026-11-17/LAY-30/).

| View | Today (`main`) | A | B |
| --- | --- | --- | --- |
| A new install, Fit, 1600 × 1000 | [folded](../qa/2026-11-17/LAY-30/mock-main-default-fit-1600.png) | [cards drawn](../qa/2026-11-17/LAY-30/mock-a-default-fit-1600.png) | as A, the bar as below |
| The same at 960 × 540 | [capture](../qa/2026-11-17/LAY-30/mock-main-default-fit-960.png) | [capture](../qa/2026-11-17/LAY-30/mock-a-default-fit-960.png) | [capture](../qa/2026-11-17/LAY-30/mock-b-default-fit-960.png) |
| The switch | [an entry of the view menu](../qa/2026-11-17/LAY-30/mock-main-menu-1600.png) | [*Checks*, on](../qa/2026-11-17/LAY-30/mock-a-bar-1600.png), [off](../qa/2026-11-17/LAY-30/mock-a-collapsed-bar-1600.png), [the board folded](../qa/2026-11-17/LAY-30/mock-a-collapsed-fit-1600.png) | [an icon, on](../qa/2026-11-17/LAY-30/mock-b-bar-1600.png), [off](../qa/2026-11-17/LAY-30/mock-b-collapsed-bar-1600.png) |
| A step's own switch | [*Expand checks here* in the Inspector](../qa/2026-11-17/LAY-30/mock-main-step-inspector-1600.png), only while the board is folded | [an arrow on the checks line](../qa/2026-11-17/LAY-30/mock-a-step-100-1600.png); [`cart` folded by it](../qa/2026-11-17/LAY-30/mock-a-step-folded-100-1600.png) | [nothing on the card](../qa/2026-11-17/LAY-30/mock-b-step-100-1600.png); the Inspector's link, in both directions |

The captures are taken before LAY-31, so the checks' lines still loop
round their cards; LAY-31 lands first.

## Questions for the owner

1. **The switch on the bar.** **A (recommended):** a button with the word
   *Checks* and an eye, highlighted while the check cards are drawn, a
   struck eye while they are folded. It can be found by reading. At
   960 × 540 the file name beside the tabs is shortened to make room
   (`promotion-and…`). **B:** an icon alone, its name on hover: about 55 px
   narrower, and the same kind of control the owner did not find in the
   menu.
2. **A step's own switch.** **A (recommended):** an arrow at the end of the
   checks line on the card, `⌄` while its checks are drawn and `›` while
   they are folded; one click, in either state of the board. The rest of
   the line still opens the list in the Inspector, where the same switch
   stays. **B:** only the link in the Inspector's *Checks* list, now in
   both directions; nothing new on the card, and the place where the owner
   did not find it.
3. **The board's switch after steps were switched one by one.**
   **A (recommended):** the board's switch sets every step: one click
   always gives a board that is all drawn or all folded, and the single
   choices made before it are forgotten. **B:** single choices survive: a
   step folded by hand stays folded when the board is switched off and on
   again, so "show all" may not show all.
4. **An installation that already has a preferences file.**
   **A (recommended):** it starts once with the checks drawn, like a new
   one; the switch is remembered from then on. The file cannot tell a
   choice from the old default: it holds `checksCollapsed: true` as soon as
   any preference was saved. **B:** the stored value is kept, so only a new
   installation changes and the owner's own computer stays folded until
   the switch is used.

## What the owner will see

- New: *Checks* on the board's bar (question 1); the arrow on a step's
  checks line (question 2).
- Changed: the Inspector's link works in both states of the board:
  *Collapse checks here* while they are drawn, *Expand checks here* while
  they are folded.
- Moved: *Collapse checks* leaves the board's view menu (the owner's
  decision).
- Default behaviour that changes: a scenario opens with its check cards
  drawn. `shop-30` at Fit in 1600 × 1000: 51 % folded, 46 % drawn (before
  LAY-31).
- Stored: a step's own choice, in `.apiforge/local/`; nothing Git tracks
  changes.

## Acceptance criteria

1. Without a preferences file, `shop-30` opens with every check card
   drawn; with a file written before this change, as question 4 decides.
2. The bar holds the switch named *Checks*, pressed while check cards are
   drawn; at 960 × 540 and 1600 × 1000 it lies within the window and
   overlaps no neighbour. The view menu has no *Collapse checks*.
3. One click on it folds every check that has a subject into its step's
   footer and a second click draws them again; no card that stays moves and
   the camera stays (LAY-11 criterion 3). The state survives a restart.
4. (Question 2 = A) The checks line of a request or component card ends
   with a button named *Collapse checks of cart* or *Expand checks of
   cart*. One click folds or draws the checks of that step only, in either
   state of the board, and selects nothing; a click on the rest of the line
   opens the *Checks* list as before. Below the zoom where the line is
   written there is no button.
5. The Inspector's *Checks* list offers *Collapse checks here* while the
   step's checks are drawn and *Expand checks here* while they are folded.
6. A step's own choice survives closing the document and the application.
   After it, `git status` in the project is clean, and `.afs` and
   `.apiforge/diagram.json` are byte for byte the same.
7. The board's switch after a step's own choice behaves as question 3
   decides.
8. A check folded by its step's own switch that fails turns the step's
   footer and border red (LAY-11 criterion 1); a gate whose result is read
   later and an `@after` stay drawn (LAY-11).
9. LAY-11's acceptance tests and `preferences.rs` state the new default;
   decision 4 of `lay.md` 5.1 is amended; the pull request prints Fit of
   `shop-30` folded and drawn.

## Not in scope

- A folded check as a far reader of a variable: LAY-35 (#410), next.
- Where a check card sits and how its lines run: LAY-31 (#384), before.
- The pill at Overview (LAY-09) gets no arrow.

## Technical appendix

- `preferences.rs`: the default, and for question 4 = A a new key (or the
  file's version) so that the stored `checksCollapsed` of an older file is
  not read; `Stored` denies unknown fields, so an older build that meets
  the new file falls back to its defaults.
- `canvas/mod.rs`: `toolbar` (the switch, the menu entry removed), `fold`
  (a step folded while the board draws its checks: the mock-up's
  `collapsed_here`), `expand_here_state`, `folded_into`; the arrow beside
  `aggregate_area` (`canvas/layout.rs`).
- `inspector/checks.rs`: `expand_here` in both directions.
- The per-document store: one JSON file under `.apiforge/local/`, by
  document path, holding the steps whose checks differ from the board's
  state and the state they were set in; an entry set in the other state,
  or naming a step that no longer exists, is dropped. With question 3 = A
  that is the whole rule: switching the board makes the earlier entries
  stale.
- Estimate at triage: S–M · 20. With the switch on the card and the store
  it is M · 25: the fold now works in two directions and a state is
  written outside the session.
