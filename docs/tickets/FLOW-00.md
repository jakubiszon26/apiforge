# FLOW-00: The flow is one chain — analysis of the ordering rules

- Date: 2026-11-16 · Analysis model: Fable 5.2
- Issue: [#388](https://github.com/jakubiszon26/apiforge/issues/388) ·
  defines RUN-03 (#389), LANG-14 (#390), LANG-13 (#386), LANG-12 (#385) and
  DIAG-06 (#391)
- Risk: `format` (what a valid `.afs` is, what the window prints), `default`
  (the order a run evaluates checks in), backward compatibility (files valid
  today become errors)
- State: **approved by the owner on 2026-11-16**, with the four consequences
  of [To confirm](#to-confirm-at-approval): RUN-03 and LANG-14 start at once,
  beside the LAY sprint; LANG-13 and LANG-12 follow LAY-12 (#296).
- **Amended on 2026-11-16 by [LANG-00](LANG-00.md#owner-decisions-2026-11-16):**
  a chain is written by the order of its lines, and `@after` leaves the
  language. That replaces decisions 2 and 8 and consequence 1 below, rules
  1–4 (now [the rules of language 2](LANG-00.md#the-rules-of-language-2))
  and the acceptance criteria of LANG-13 and LANG-12; LANG-12 is not
  needed. Decisions 1 and 3–7, rules 5 and 6, RUN-03 and LANG-14 stand.

## Goal

The owner's finding on a real board (2026-11-16): *Start* sends four dashed
lines to four steps and nothing says which runs first; "`@after` should be
mandatory, and two `@after(begin)` make no sense". After these tickets the
steps of a scenario form **one chain**: every step says what it follows,
nothing is followed by two steps, only an `if` splits the flow, and it joins
again after it. The order can be read top to bottom in the code and followed
along one line on the board.

## What happens today

Run on `main` (c7e9313) with `apiforge check` and the simulation of
`apiforge run`; the five files are in the
[appendix](#the-five-experiments).

| # | Fact | Evidence |
| --- | --- | --- |
| F1 | Several statements may follow the same step. They run **in the order they are written**, although the reference says "line order means nothing" | `zeta`, `alpha`, `mid`, all `@after(begin)`, ran `zeta → alpha → mid`; `engine.rs`: "among ready nodes the order of `graph.nodes` decides" |
| F2 | A request that nothing orders after the start **does not run, and nothing says so**: `check` is silent, the run reports the step *skipped* and ends *passed* | `loose` in the same file |
| F3 | Two requests inside a branch's braces both follow the branch and run in file order | `second`, `first` ran in that order |
| F4 | A second step in a branch written `@after(first)` runs in a chain, but the window prints it **outside** the braces as `@after(created.then, first)` | `apply`, then `print` |
| F5 | `@after(check)` only orders: with `on_error continue` the step after a **failed** check still runs. Without it, whether a check is evaluated before the next request depends on where its line is | `ok` failed, `gated` passed |
| F6 | A file without a `start` runs every statement in file order | `b`, `a` |
| F7 | In the repository's twelve `.afs` files no step is followed by two steps explicitly, but **39 of 103** requests, calls and `if`s outside braces carry no `@after` (they are ordered by the values they read), in nine files | counted by script |
| F8 | **34 of the 52** `@after` name a check (`@after(cart_total)` on the next request); every `@after` is written on a request, none on a check or a value | counted by script |

## The rules

A **step** is a statement that does something or routes the flow: `start`,
a request, a component call, an `if`. Everything else — a value, `var`,
`assert`, `condition`, `export` — is a **data statement**.

1. **A flow with a step has a `start`.** The first step writes
   `@after(begin)`.
2. **Every request, component call and `if` writes `@after(…)`.** Inside
   braces — a branch of an `if`, the `cleanup` — the first step writes
   nothing, the braces place it; every further step of the same braces
   writes `@after`.
3. **`@after` belongs to steps.** It is written on a step and names a step:
   the start, a request, a call, or an `if`. A check or a value is placed by
   what it reads, or by the braces it stands in.
4. **Nothing is followed by two steps.** A step, the start and an `if` are
   each named in at most one `@after`; a pair of braces holds at most one
   step without `@after`.
5. **`@after(x)` on an `if` means "after the whole `if`".** The step runs
   when the branch that was taken has finished its last step, or at once
   when that branch holds no step.
6. **Checks and values run before the next step.** When a step has
   finished, everything that can be computed from what has run so far is
   computed, then the next step starts.

```afs
scenario "Buy coffee" {
  begin = start
  @after(begin)
  cart = POST "/carts" { … }
  created = assert cart.status == 201      // runs before `paid`, rule 6
  @after(cart)
  paid = if cart.body.total > 0 {
    payment = POST "/payments" { … }       // first in the braces, rule 2
    @after(payment)
    receipt = GET "/receipts/{{payment.body.id}}" { … }
  }
  @after(paid)                             // after the whole if, rule 5
  logout = POST "/logout" { … }
}
```

## Owner decisions (2026-11-16)

Taken in a quiz, the recommended option first; all eight are the
recommendation.

| # | Question | Decision |
| --- | --- | --- |
| 1 | What is an error: several steps after one, or several names in one `@after` | **One successor.** A step is followed by at most one step; an `if` is the only split |
| 2 | Which statements must write `@after` | **Requests, component calls and `if`.** Checks and values stay ordered by what they read; a step without a place is an error, not a silent skip |
| 3 | A check before the next step | **The run computes ready checks and values before the next step**; `@after(check)` becomes an error |
| 4 | The first step | **`start` is mandatory**; in the cleanup and in a branch the braces place the first step |
| 5 | Existing files | **An error at once, with a one-click fix** that writes the chain in the order the file runs today; the same for a whole project from the command line |
| 6 | The board: a second ordering wire from a *Next* port | **It inserts into the chain**: the new step follows the port's step, the former successor follows the new step (DIAG-06, with a mock-up) |
| 7 | A step after an `if` | **`@after(x)` = after the whole `if`**; the join by the ends of both branches is no longer needed |
| 8 | `@after` on a check or a value | **No**: `@after` is written on steps only |

## What the owner will see

- New: errors in *Problems*, on the command line and for an agent,
  each at its statement and with a fix: a step that does not say what it
  follows; a second step after the same step; `@after` on or to a check or
  value. A fix for the whole file, *Chain the steps in the order they run
  today*.
- New: `@after(x)` after an `if`.
- Changed: the window prints a chain inside a branch inside its braces
  (today it moves the second step out, F4).
- Changed: a run evaluates a step's checks before the next request. Under
  `on_error stop` a failed check now stops the run **before** the next
  request is sent; today that depends on the order of the lines (F5).
- Default behaviour that changes: a file with a request and no `start`, a
  request without `@after`, two steps after one step, `@after(check)` — all
  valid today — do not pass the check and do not run until fixed. A request
  that runs nowhere (F2) is an error instead of a silent skip.
- On the board (follows from the rules, no ticket of its own): the fan of
  dashed lines from *Start* (#382) cannot occur; every ordering wire goes
  into the next step, so LAY-24's short arrow replaces nearly every dashed
  rail; badge numbers read step, its checks, next step.
- Removed or hidden: nothing on screen. From the language: `@after(a, b)`
  and `@after(check)` ([to confirm](#to-confirm-at-approval), 1).
- Migrated or converted: nothing automatically. The fix is applied by the
  person, per file or per project (decision 5).

## To confirm at approval

Four consequences the quiz did not ask about. Each is the planning model's
recommendation and the owner's to veto.

1. **`@after` takes one name.** With rule 5 the only use of
   `@after(a, b)` — joining the branches of an `if` — is `@after(x)`; the
   fix rewrites the old joins. A second name is an error.
2. **What is already published keeps running.** A published component
   revision is immutable; it is not checked again and runs as it does
   today, apart from rule 6. The legacy JSON workspace (sunset, KTLO-06)
   is not checked against rules 1–5 either.
3. **The LAY reference scenarios are rewritten.** `shop-30`, `shop-50`,
   `order-flow`, `fan-out` and the `far-value` pair need the fix (F7, F8),
   and their ordering wires then run step to step instead of check to
   step, which moves the LAY-01 figures. LANG-13 therefore lands **after
   the LAY checkpoint** (#296), or the baseline is measured again in its
   pull request. RUN-03 and LANG-14 change no file and can land earlier.
4. **The fix cannot place everything.** A step that runs nowhere today
   (F2), and an ordering that is not a chain with `if`s (a step after
   branches of two different `if`s), stay errors for the person to place.

## Tickets, order and estimates

| Order | Ticket | Delivers | Estimate |
| --- | --- | --- | --- |
| 1 | **RUN-03** #389 | Rule 6 in the runner. No file changes; `@after(check)` still accepted | M · 20 min · `default` |
| 2 | **LANG-14** #390 | Rule 5 and the chain printed inside braces. Additions only: nothing valid becomes invalid | S–M · 20 min · `format` |
| 3 | **LANG-13** #386 | Rules 1 and 2 as errors; the fix, which already writes the final form (one chain, `@after(x)` joins, no check named), so a file is rewritten once; the reference; the repository's scenarios fixed | M · 30 min · `format` |
| 4 | **LANG-12** #385 | Rules 3 and 4 as errors, for what the fix did not write; `@after` takes one name | S–M · 20 min · `format` |
| 5 | **DIAG-06** #391 | The board's gestures keep the chain. Mock-up first; analysed and estimated from it | — |

About 90 owner minutes without DIAG-06. Each is `M` or below because the
risk is bounded by the fix: it is tested to keep the order of steps of every
repository scenario, and the simulated run of each scenario before and after
is compared in the pull request.

Between LANG-12 and DIAG-06 the board can still draw a second ordering wire
or drop a step without one; the file then shows the error and the fix.
DIAG-06 follows directly.

### Acceptance criteria

**RUN-03**

1. In a file where a request's line comes before a check of the previous
   step, the simulated and the live run evaluate the check first.
2. Under `on_error stop` a failed check stops the run before the next
   request: the request's step is *not run* and nothing is sent.
3. The order of steps (requests, calls, `if`s) of every repository scenario
   is the same before and after; only data statements move earlier.
4. The same holds inside a component and in the cleanup.

**LANG-14**

1. `@after(x)` on an `if` runs the step after the last step of the branch
   taken, for `then`, for `else`, and for a branch without a step.
2. A step after `@after(x)` does not run when the branch taken ended in a
   step that was blocked or errored, as after any such step today.
3. A statement in a branch's braces that follows a statement of the same
   braces is printed inside them by the window and kept there by
   `apiforge fmt`; text → window → text is unchanged.
4. The reference describes both; `@after(x.then)` and `@after(x.else)`
   keep today's meaning.

**LANG-13**

1. A request, call or `if` outside braces without `@after` is an error at
   its statement that names the step it would follow today.
2. A second step without `@after` in the same braces is an error.
3. A main flow with a step and no `start` is an error.
4. The fix rewrites a file so that the steps run in the order they run
   today; a file it cannot place completely keeps an error at each step
   left, and says so.
5. One command applies the fix to every file of a project and prints what
   it changed; an agent gets the same errors and the rule in the language
   reference.
6. Every `.afs` in the repository passes; the simulated run of each lists
   its steps in the same order as before.
7. A published revision that breaks the rules still runs.

**LANG-12**

1. A step, the start or an `if` named in two `@after` is an error at the
   second, with the fix that chains them in file order.
2. `@after` on a check or value, and a check or value named in `@after`,
   are errors with a fix.
3. `@after(a, b)` is an error; the fix writes `@after(x)` when `a` and `b`
   are the ends of `x`'s branches.
4. The reference's sentence "line order means nothing" holds for steps:
   reordering the lines of any repository scenario changes neither the
   order of its steps nor its result.

## Not in scope

- What a failed step does to the steps after it (RUN-01, #126), repeating
  a step (LANG-08, #352), a value from whichever branch ran (LANG-09,
  #353).
- The board's gestures (DIAG-06) and how ordering wires are drawn
  (LAY-24, #382).
- Running steps in parallel: the runner stays serial.

## Technical appendix

### The five experiments

Each applied to a copy of `fixtures/workspace.json` and simulated
(`apiforge apply`, `apiforge run --json`); samples omitted here.

```text
t1  begin = start; @after(begin) zeta; @after(begin) alpha; @after(begin) mid; loose (no @after)
    → begin, zeta, alpha, mid passed; loose skipped; run passed; check silent
t2  created = if … { second; first } else { other }; @after(first, created.else) joined
    → created, second, first, joined passed; other skipped
t2b created = if … { first; @after(first) second }; @after(second, created.else) joined
    → created, first, second, joined; print: second outside the braces as @after(created.then, first)
t3  on_error continue; ok = assert cart.status == 201 (fails); @after(ok) gated; @after(cart) plain; uses (reads cart)
    → cart passed, ok failed, gated, plain, uses passed
t5  no start: b, a → b, a
```

### Where the rules live

- Rule 6: `apiforge-runner/src/engine.rs`, the choice among ready nodes —
  data kinds (`Value`, `Assert`, `Condition`, `Export`) before step kinds,
  file order within each. `plan.rs` is unchanged.
- Rules 1–4: `apiforge-core/src/validation.rs` beside the ordering-edge
  checks #377 touched, on source documents (check, save, publish); a
  stored revision is not validated again when it is called.
- Rule 5: `apiforge-script` `lower.rs` (an edge from the end of each branch;
  the end is the last step of the braces' chain, or the branch itself) and
  `print.rs` (the short form when a step follows exactly those ends).
- Braces membership in `print.rs`: today "a statement whose only ordering
  is one branch"; it becomes "every ordering of the statement is the branch
  or a statement of the same braces". `lower.rs` keeps adding the branch
  edge to every statement in the braces, so the engine sees what it sees
  today.
- The fix computes the order the scheduler would take — a topological
  order with file-order ties, per braces — without running the file.

### Open technical questions

- The command for a whole project: a flag of `apiforge fmt` or of
  `apiforge check --folder`. Decided in LANG-13 with CLI-01 (#359).
- Whether `check_text` returns the fixed text to an agent, or only the
  message and the reference.
- `fixtures/runs` and the acceptance tests that assert a step order with a
  check between two requests: listed in RUN-03's pull request.
- `@after(x.then)` written outside the braces stays valid as the first
  step of that branch; whether the language still needs it is left to a
  later clean-up.
