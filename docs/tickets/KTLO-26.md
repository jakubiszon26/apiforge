# KTLO-26: A board edit keeps the model the printed source reads back

- Size: S · Owner review: 10 min · Risk: `format` (raised for the analysis;
  the decision changes no format)
- State: Awaiting acceptance (issue #79); decision A taken 2026-10-27 on #79
- Actual size: **579** changed lines (574 added, 5 removed) against `main` at
  `0586860`, measured at the docs commit: the fix 24 (19 added, 5 removed),
  the acceptance tests 381 (321 of them the analysis's reproduction), this
  ticket, the QA report and the backlog row 174. Over the 300 of S and inside
  the 800 of M, so **M** measured and not split: the code is 19 added lines,
  the rest is the reproduction the analysis wrote and the written evidence.
  1 round.
- Analysis model: Fable · Implementation model: Opus 5.5
- Suspected while fixing KTLO-03 (#5); reproduced on the spike branch
  `spike/ktlo-26-rewire-digest` (`4ec99c9`, kept on origin).

## Goal

A board edit that changes what a scenario or component does, then a save,
seals a checkpoint that describes the saved source, so the next load restores
the saved model exactly instead of reconciling against it.

## What the owner will see

- New, changed on screen, removed, hidden, migrated or default behaviour:
  nothing. No block moves, changes identity or loses its selection; the
  source the board prints is the same text as before.
- What changes is internal: right after such a board edit, the model in the
  session lists its blocks and each block's inputs in the order the printed
  source states, which is the order a reload of that source gives. Before,
  the model kept the board's order until the document was reloaded, and the
  checkpoint saved from it did not describe its source.

## Cause

After a board edit that deletes the request feeding a check and wires a new
one in, the saved checkpoint's semantic digest (`0e5d5b72…`) differs from what
the saved source lowers to (`41551dde…`): `delete_nodes` drops `actual`,
`connect` appends it after `expected`, the printer always prints `actual` then
`expected`, and `install_model` (`session.rs:755-776`) printed the text but
kept the edited model unchanged, so `save` sealed the stored order. The typed
source path is safe. On reload nothing visible is lost: `derive` falls back to
reconciling by handle, so every id and position survives; only the order of
blocks and inputs in the model differs, and the next save of that document
writes a record that describes the source. KTLO-03 saw a refusal only because
migration's `verify` compares exactly.

## Decision

**A — fix in the session** (owner, 2026-10-27 on #79). When a model edit
changes semantics, `install_model` derives the model from the printed text
with the edited model as the identity base
(`analysis::derive(&target, &text, &self.context, Some(&model), None, placer)`),
so identities and positions are the edited model's and the order is the
source's. If the printed text does not read back, the edited model is kept as
before. No format change, no digest change, existing checkpoints untouched.

Declined: **B — an order-independent semantic digest**, which changes the
checkpoint format and every existing record's digest.

## Acceptance criteria

1. After the board edit of the reproduction (delete the request feeding the
   cleanup check, add a request, wire its status into `actual`), the session's
   model keeps every block identity and position and every order-edge
   identity of the edited model, and lists the check's inputs as
   `actual, expected`.
2. The checkpoint sealed by the next save describes the saved source; the
   reload restores the saved model exactly, with the same block and input
   order as the session had after the edit.
3. The same rewiring typed as source still seals a checkpoint that describes
   it.
4. The desktop's board edits (`folder/publication.rs`, the Diagram mode) see
   no change in identities, positions or selection: the existing desktop
   tests pass unchanged.
5. No format or digest change: `apiforge-storage` is untouched and the
   fixture check passes; the verification commands of the development guide
   pass.

Criteria 1–3 are the two KTLO-26 tests in
`crates/apiforge-project/tests/acceptance.rs`.

## Not in scope

Migration's exact comparison (KTLO-03, decision 6 on #5); checkpoints already
sealed in the board's order, which the next save of their document rewrites.
