# LAY-08: Check subjects in core, and the Inspector's list of a step's checks

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: Awaiting acceptance (issue #292) · decided by the owner on 2026-11-05 (Q6 = A
  not vetoed; the gate rule of [lay.md 4.7](../sprints/lay.md#47-second-opinion-on-round-3-and-the-four-proofs)
  item 4) · LAY sprint, wave 0 (no board change)
- Analysis model: Fable · Implementation model: Opus

## Goal

The model knows which step a check verifies, so every later ticket (badges,
collapse, focus, the Inspector) can say it in words: a request's Inspector
lists its checks with their sources, status and moment, and a check with two
subjects is counted under both.

## What the owner will see

- New, in `apiforge-core`: a function that names the subjects of a check —
  the steps whose outputs it reads directly — and a `sequence` function that
  replays the engine's order without executing (the static scheduler of the
  plan's appendix), which the moment word and LAY-07's badges read. Not
  visible.
- New, in the Inspector of a request: a *Checks* section listing its checks
  in words — the predicate (`cart.total ≈ total`), the sources, the status
  and the moment (`Waiting for receipt · Shared` for a check that also reads
  a later step). Gates (a *Data condition* whose value another step reads)
  are counted apart, as *Gate*.
- Changed, removed, hidden: nothing. Default behaviour that changes: none.

## Decisions taken (owner, 2026-11-05)

> Q6 = A (recommended, not vetoed): a check with several subjects is
> counted under every subject it reads directly; the Inspector list shows
> it under each with "also checks receipt" (one line *Shared with: cart*).

The gate rule is part of the owner's Q4 = A (its option text: a gate whose
result is read later is never hidden); the planning model stated it in 4.7
item 4. LAY-08 marks gates so LAY-11 can keep them visible.

Decided on acceptance (2026-11-05):

- The list is a third *Checks* tab of a request's Inspector, not a section:
  a section at the end of *Configuration* fails criterion 5, and one at its
  top moves existing fields.
- Criterion 5 holds before a run. After a run at 960 × 540 the Run results
  bar takes room and the list scrolls; that is accepted.
- `check_subjects` returns subjects and value sources separately, and a value
  never makes a check shared. This is how criterion 1's "returns both" is
  read: `cart` is the subject and `expected_total` a value source, as in
  [lay.md 1.3](../sprints/lay.md#13-facts-about-the-model-that-the-design-must-respect).
- A component call's checks and a Cleanup check reading a Main step are not
  listed. LAY-20 (#308) covers them and must close before LAY-11.

## Mock-up

Round 3 of LAY-00: `lay00r3-inspector-checks`, `lay00r3-inspector-timing`
(and their `-window` frames) on `mock/lay-00-round-3` at `ea981d8`, posted to
#37. Approved as direction on 2026-11-05.

## Acceptance criteria

Core tests in `crates/apiforge-core/src/graph.rs` (`mod tests`), window
tests in `crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod
lay08`, on `shop-30.afs`.

1. `check_subjects` of a check reading `cart.total` and `expected_total`
   returns both; of a standalone check (no step output read) returns none;
   of a *Data condition* read by a later step returns its subjects and marks
   it a gate. Test: `check_subjects_names_every_step_read_directly`.
2. The Inspector of `cart` lists its checks with predicate, sources, status
   and moment; the two-subject check appears under `cart` and under
   `receipt`, with *Waiting for receipt · Shared* under `cart` and *Shared
   with: cart* under `receipt`. Test:
   `lay08_criterion_2_the_inspector_lists_the_checks_in_words`.
3. A *Data condition* whose value a later step reads is listed as *Gate*
   under its subject and is not counted among the checks. Test:
   `lay08_criterion_3_a_gate_is_counted_apart`.
4. A standalone check is listed under no step. Test:
   `lay08_criterion_4_a_standalone_check_is_listed_nowhere`.
6. `sequence` agrees with the runner's order on the three fixtures (the
   runner's loop, `engine.rs:518–560`, is the reference). Test:
   `sequence_agrees_with_the_engine_on_the_fixtures` in core.
5. The list fits the Inspector at 960 × 540 without clipping its first
   three rows. Test: `lay08_criterion_5_fits_the_small_window`.

## Not in scope

Hiding checks (LAY-11); the footer aggregate (LAY-11); the check card's
subtitle (LAY-09); the badges (LAY-07); timing rows in the list beyond the
moment word; MCP's check tool using the function (later).

## Technical appendix

- Subjects: `Binding::references()` over `node.inputs`, filtered by node
  kind, beside `dependencies` in `apiforge-core::graph`; a `Subject { step,
  shared: bool, gate: bool }` result.
- Sequence: `apiforge_core::graph::sequence` orders the steps by the
  engine's rule (dependencies, `@after`, branches) without executing;
  `dependencies` alone gives no order, so the function is this ticket's.
- Inspector: a section in `folder/inspector.rs` for request nodes, built
  from `check_subjects` of every check in the graph; the moment word
  (*Waiting for receipt*) compares the check's `sequence` position with its
  subjects'.
