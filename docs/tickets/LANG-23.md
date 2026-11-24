# LANG-23: Seven findings about the language reference — what is true, why, and what to change

- Size: analysis S · Owner review: 15 min · Risk: none in this file (documentation);
  the changes it proposes carry `format` and `default` and are the owner's
- State: **Decided** (2026-11-24). The owner took decisions 1, 2 and 4 and
  delegated 3 to an independent agent; see [Decisions](#decisions). The
  findings below are the analysis as written at `fe74ae73`; where a decision
  or a later check corrects one, the Decisions section says so.
- LANG sprint, second half (part B) · branch `docs/lang-23-reference-findings`

## Goal

A functional tester reads [the reference](../language/reference.md) top to
bottom and writes a scenario from it. Seven places make that harder than it
needs to be: one says two things, four describe a rule that surprises, two
are small. This file says for each whether it is true, where it comes from,
what already covers it, and what to do, ranked by what a tester gains.

## Summary, ranked by value to the tester

| # | Finding | True today | Origin | Covered by | Recommendation |
|---|---|---|---|---|---|
| F4 | `&&` and `||` evaluate both sides, so a guard does not protect the read after it | yes | history (operators are blocks with two inputs) | partly: LANG-16 D5 (`??`), LANG-01 #127 | **Language change**, decided inside the LANG-01/LANG-09 analysis |
| F5 | A check runs when its values exist, wherever its line stands | yes | deliberate (LANG-13, FLOW-00) | LANG-13; on the board LAY-43 #466 | **Keep the rule**; reference wording plus a new warning |
| F1 | The reference says a component needs a `start` and that it needs none | yes | an edit missed when LANG-13 withdrew the rule | — | **Docs only**, now |
| F3 | `input x` is read as `param.x`; `argument x` is read as `input.x` | yes | history (ADR 0004 names, kept by ADR 0005) | — | **Language change** with a fix, or docs only: owner |
| F2 | A scenario must write `begin = start` although nothing reads `begin` | yes | deliberate for 0.2.0 (LANG-13, consequence 1) | — | **Language change**: `start` becomes optional; owner |
| F6b | One export is read directly, two by name, so a second export changes how a caller reads the first | yes | history (single-output blocks) | — | **Additive change**: the name is always accepted |
| F6a | Nothing negates a computed value | yes | deliberate (ADR 0005) | — | **Additive change**, small; with LANG-03 |

## F1. A component and `start`: the reference contradicts itself

- **Evidence.** *Start and ordering* says "a scenario or a component with a
  step has a `start`" (`reference.md:138-139`). *Diagnostics* says "A
  component needs no `start`: its body runs from its first step written"
  (`reference.md:814-815`). The code agrees with the second: the rule runs
  only for a scenario (`apiforge-script/src/lower.rs:128-134`), and the
  message speaks of a scenario (`fix.rs:23`).
- **Why.** LANG-13 first asked a `start` of a component and withdrew it on
  2026-11-20 ([LANG-13](LANG-13.md), consequence 1 and A3). The sentence in
  *Start and ordering* was not rewritten. Every component example in the
  reference still writes `begin = start`, which is valid and misleading.
- **Recommendation: docs only.** Rewrite the sentence; drop `begin = start`
  from the component examples (the reference test runs them, so the change
  proves itself). No language change. `ready:sonnet` material once approved.

## F2. `begin = start` in a scenario

- **Evidence.** A scenario with a step and no `start` is an error with a fix
  that writes the line (`lower.rs:339-360`, `fix.rs:23`). The handle is
  never read by another statement since `@after` left the language.
- **Why.** Deliberate for 0.2.0: the runner reaches steps from the Start
  block ([ADR 0003](../decisions/0003-start-reachability.md)), the board
  shows it, and LANG-13 chose the smallest change to the graph. A component
  was exempted because the board offers no *Start* there; the same argument
  now holds for the text of a scenario.
- **Recommendation: language change, the owner's.** The lowering supplies
  the Start block when a scenario writes none, as it already does in
  substance for a component; a written `start` stays valid. Additive for
  files, but a file without `start` is refused by 0.2.0, so by
  [LANG-00, section 2](LANG-00.md#2-the-language-version-lang-15) the
  language version rises (*to confirm* against that section). Migration:
  none; the formatter may drop the line only if the owner asks. Touches the
  board (Start card of a scenario without the line): needs a mock-up.

## F3. Declared under one word, read under another

- **Evidence.** A component declares `input productId` and reads
  `param.productId` (`reference.md:104-115`); a request declares
  `argument cart` and reads `input.cart` (`reference.md:192-193, 224-225`).
  The diagnostics follow: *Unknown parameter* for a missing `input`,
  *Unknown request argument* for `input.x` (`reference.md:776-781`).
- **Why.** History. `input.name` is the reading of a request's `argument`
  leaf in [ADR 0004](../decisions/0004-document-format-v2.md) (named request
  arguments, 23 October 2026), and `param.name` the `parameter` leaf;
  [ADR 0005](../decisions/0005-scenario-language.md) (`:228-229`) kept both
  when components gained `input` declarations.
- **Options.**
  - **A (recommended).** A request's argument is read as `arg.name`;
    `input.name` stays readable and `apiforge fix` rewrites it. `param.`
    stays for what a file receives from outside, scenario or component, and
    the reference says so in one sentence. One word changes, `input.` stops
    meaning two things, no stored document changes (the leaf is the same).
  - **B.** Also read a component's inputs as `input.name`. Reads best, but
    `input.` would change meaning between language versions; a file read
    under the wrong version would resolve silently to the other thing.
  - **C.** Docs only: a table "declared as / read as" in *Names*.
- **Cost of A.** Parser, printer, completion, the fix, the MCP reference;
  M. The version rises only when `input.name` in a request is refused, which
  A does not ask for.

## F4. `&&` and `||` evaluate both sides

- **Evidence.** The reference states it (`reference.md:304-305`). The
  operator takes two computed values
  (`apiforge-core/src/operator.rs:406-408`, with the comment "Both operands
  are always evaluated"). A logical operator is a block with two inputs, so
  an operand that reads a missing field is an error before the operator
  runs (*to confirm* with a test: `exists x.body.coupon &&
  x.body.coupon.discount > 0` on a response without `coupon`).
- **Why.** History: the operators were blocks on the board before there was
  text, and a block waits for all its inputs.
- **Covered.** In part. LANG-16 D5 = A (#413): a check on a missing field
  fails and names it, and `??` gives the stand-in, so the common case is
  written `x.body.coupon.discount ?? 0 > 0`. LANG-01 #127 is the ticket.
  Neither makes `a && b` skip `b`, and a tester who knows any other
  language will write the guard.
- **Recommendation: language change, decided in the LANG-01/LANG-09
  analysis (row 2 of part B), not on its own.** Preferred shape: the right
  side of `&&` is not needed when the left is false, of `||` when the left
  is true; a missing value on the unneeded side is not an error. Behaviour
  changes only for files that end in an error today, so no passing scenario
  changes its result; the version rises (a default of evaluation changes).
  The board keeps one block with two inputs. If the owner prefers to keep
  the rule, `??` must be in the same release and the reference must show
  the guard written with it.

## F5. A check written last may run first

- **Evidence.** "Checks and values are not steps: each runs as soon as the
  values it reads exist, before the next step starts, wherever its line
  stands" (`reference.md:145-147`); only steps are chained
  (`apiforge-core/src/chain.rs:25`).
- **Why.** Deliberate: FLOW-00 and [LANG-13](LANG-13.md) order steps by the
  text and leave values and checks to their data. It keeps a failed check
  beside the response it judged, and with a stop on error
  ([ADR 0015](../decisions/0015-run-after-an-error.md)) the run ends at the
  step whose check failed, not later. The board draws a check where it runs
  (LAY-43 #466).
- **Recommendation: keep the rule; change what the tester sees.**
  1. Reference: one example in *Start and ordering* with a check written
     late, and the sentence that the formatter never moves it.
  2. **Proposal, new:** a warning *This check runs after `<step>`, written
     N steps above* with a fix that moves the line below that step. LANG-13
     has the same idea for steps (*This statement is not written where it
     runs today*); whether a warning for checks already exists is *to
     confirm*. No language change, no version.

## F6a. No unary minus

- **Evidence.** `reference.md:303-304`; the parser accepts `-` only before
  a number literal (`parser.rs:750-751`).
- **Why.** Deliberate: "no operator negates a value"
  ([ADR 0005](../decisions/0005-scenario-language.md), `:232`); the grammar
  has `!` as the only unary (`:205`).
- **Recommendation: additive, small.** `-x` lowers to the subtraction
  `0 - x` and prints back as `-x` when the left side is the literal `0`.
  Owner's question hidden in it: a scenario that writes `0 - x` today would
  be reprinted as `-x`. Ride with LANG-03 (text functions, S, additive).

## F6b. One export or two changes how a caller reads them

- **Evidence.** "A component with exactly one export has a single output,
  read directly" (`reference.md:622-624`); a block with a single output
  port takes a path directly, otherwise the first segment names the port
  (`apiforge-core/src/template_text.rs:4-6`).
- **Why.** History: the rule of single-output blocks, applied to a
  component call. A call pins its revision, so nothing breaks until a
  caller moves to the revision with the second export; then every read of
  the first export is an error to fix by hand.
- **Recommendation: additive.** `call.exportName` is accepted for a
  component with one export too, the reference and the completion write
  the name always, and the direct form stays valid. One ambiguity to rule
  on: a single export whose value has a field of the export's own name.
  No version rise if 0.2.0 files stay valid; a file using the new spelling
  needs the newer build.

## Proposals beyond the seven

- **P1. The reference examples lead by the shortest valid file.** After F1
  and F2 the first example of each kind has no `begin = start`.
- **P2. A "coming from code" page** in the reference: the five rules that
  differ from a general-purpose language (checks run by data; both sides;
  no negation; strict equality; text never becomes a number), each with the
  spelling to use instead. Docs only; removes most of the surprise of F4–F6
  even before any change.
- **P3. One language step for F2, F3 and F4** rather than three, so a
  project is updated once (LANG-20's fix carries F3).

## Decisions

Taken on #589, 2026-11-24.

1. **F4: short-circuit** in `&&` and `||`, designed in the LANG-01 #127 /
   LANG-09 #353 analysis; the language version rises there. *Confirmed by a
   simulation on 0.2.1:* `assert exists cart.body.coupon &&
   cart.body.coupon.discount > 0` on a response without `coupon` ends in
   *error*: "and: greater: Operand `left` of Greater expects number, got
   missing." That scenario is the first acceptance test of the change.
2. **F3: option A.** A request's argument is read as `arg.name`;
   `input.name` stays readable and `apiforge fix` rewrites it; `param.` is
   unchanged. Ticket: [LANG-25](LANG-25.md) (#592).
3. **F2: `start` stays required in a scenario** (ruled by an independent
   agent on the owner's delegation; reasons on #589).
   - The version claim of F2 above is **refuted**: by LANG-00 section 2,
     rule 1, the number rises only when a valid file means something else
     or becomes invalid. An optional `start` does neither; 0.2.x refuses
     such a file at its line, as it does `uuid()`. So the relaxation costs
     no version and no migration **whenever** it is made, and nothing is
     gained by making it in the same step as F3 and F4 (P3 does not apply
     to F2).
   - It is not free of the owner's decisions: either the scenario has no
     Start card (the board changes), or the lowering supplies one and the
     window then writes a line the person did not write, or a mark of
     "written or not" is stored (format). Each needs a mock-up or a
     decision; none is settled by the text alone.
   - The gain is one line per scenario that the fix already writes; the
     rule is four days old (LANG-13, consequence 1), and the 0.1.0 reading
     of a pasted file is judged only because the line is missing
     (`lower.rs`, `fix::run_order`).
   - Relaxing later stays open and cheap; requiring it again after a
     relaxation would raise the version. Reopened only with evidence that
     testers trip on it (an A/B round), and then with a mock-up of the
     board for the owner. No ticket is cut.
4. **Documentation now**, as one `ready:sonnet` ticket: LANG-24 (#591).
   F5.1 is smaller than written above: the example with a check written
   late is already in *Start and ordering*; one sentence about the
   formatter is added.

Cut from the rest: F6a rides with LANG-03 (#131, comment); F6b is
candidate LANG-26 (#593) and the warning of F5.2 candidate LANG-27 (#594),
both for the owner's decision. No warning for a check written away from
its step exists in `apiforge-script` today.

## Not in scope

Any change to the code, to the reference or to an ADR; the shape of `??`
(LANG-01); loops and blocks (LANG-16).
