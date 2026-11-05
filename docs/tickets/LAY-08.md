# LAY-08: Check subjects in core, and the Inspector's list of a step's checks

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: Backlog (issue #292) · decided by the owner on 2026-11-05 (Q6 = A
  not vetoed; the gate rule of [lay.md 4.7](../sprints/lay.md#47-second-opinion-on-round-3-and-the-four-proofs)
  item 4) · LAY sprint, wave 0 (no board change; may start now)
- Analysis model: Fable · Implementation model: Opus

## Goal

The model knows which step a check verifies, so every later ticket (badges,
collapse, focus, the Inspector) can say it in words: a request's Inspector
lists its checks with their sources, status and moment, and a check with two
subjects is counted under both.

## What the owner will see

- New, in `apiforge-core`: a function that names the subjects of a check —
  the steps whose outputs it reads directly. Not visible.
- New, in the Inspector of a request: a *Checks* section listing its checks
  in words — the predicate (`cart.total ≈ total`), the sources, the status
  and the moment (`Waiting for receipt · Shared` for a check that also reads
  a later step). Gates (a *Data condition* whose value another step reads)
  are counted apart, as *Gate*.
- New: a check card's subtitle names its subject and predicate.
- Changed, removed, hidden: nothing. Default behaviour that changes: none.

## Decisions taken (owner, 2026-11-05)

> Q6 = A (recommended, not vetoed): a check with several subjects is
> counted under every subject it reads directly; the Inspector list shows
> it under each with "also checks receipt" (one line *Shared with: cart*).

Decided by the planning model as a rule of the model (4.7 item 4): a gate
whose result is read later is never hidden by collapsing; LAY-08 marks it so
LAY-11 can keep it visible.

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
4. A standalone check is listed under no step; the check card's subtitle
   reads subject and predicate. Test:
   `lay08_criterion_4_standalone_and_subtitle`.
5. The list fits the Inspector at 960 × 540 without clipping its first
   three rows. Test: `lay08_criterion_5_fits_the_small_window`.

## Not in scope

Hiding checks (LAY-11); the footer aggregate (LAY-11); timing rows in the
list beyond the moment word; MCP's check tool using the function (later).

## Technical appendix

- Subjects: `Binding::references()` over `node.inputs`, filtered by node
  kind, beside `dependencies` in `apiforge-core::graph`; a `Subject { step,
  shared: bool, gate: bool }` result.
- Inspector: a section in `folder/inspector.rs` for request nodes, built
  from `check_subjects` of every check in the graph; the moment word comes
  from the engine order (LAY-07's sequence function once merged; until then
  from `dependencies`).
