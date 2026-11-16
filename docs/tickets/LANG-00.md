# LANG-00: The ground under the one-chain language (part A)

- Date: 2026-11-16 · Analysis model: Fable 5.2
- Issue: [#411](https://github.com/jakubiszon26/apiforge/issues/411) ·
  defines RUN-01 (#126) and LANG-15 (#412); places LAY-34 (#396), LANG-06
  (#143), KTLO-37 (#211) and EDIT-03 (#369); may change LANG-13 (#386),
  LANG-12 (#385) and LANG-14 (#390)
- Risk: `default` (what runs after an error), `format` (the manifest, what a
  valid `.afs` is)
- State: **analysis, waits for the owner**: three decisions in
  [Owner decisions needed](#owner-decisions-needed), four consequences in
  [To confirm at approval](#to-confirm-at-approval)
- Part B (loops, block results, scopes, the board) is LANG-16 (#413)

## Goal

[FLOW-00](FLOW-00.md) made the steps of a scenario one chain. Four things
under it were left open, and LANG-13 rewrites every file, so they are
settled first: what the chain does after an error, how a project says which
language it is written in, which operation may change which part of a
file's text, and whether a chain is written with `@after` or by the order
of its lines.

The owner's frame (2026-11-16): breaking changes are allowed in the 0.x
series; no build reads several language versions, there is no migration
framework and no rollback. The version exists to **detect** a mismatch.
Simple for a tester, deterministic, native on the board, safe for an agent
to edit.

## 1. What a chain edge means after an error (RUN-01)

Decided by the owner (#388, decision 1): the edge means "after it
finished"; with `on_error continue` only missing data blocks the next step;
with `stop`, the default, the run stops as today; no new syntax.

### Measured on `main` (2026-11-16, simulated, `on_error continue`)

| # | File | Today | With the rules below |
| --- | --- | --- | --- |
| M1 | `a` errors (no response); `@after(a) b` reads `a.body.id`; `@after(b) c` reads nothing; `seen = assert c.status == 200` | `b`, `c`, `seen` all *blocked* | `b` blocked by `a`; **`c` runs**, `seen` passes; the run is *failed* |
| M2 | `a` answers 500 with `{ error, requestId }`; `b` reads `a.body.id`; `@after(b) trace` reads `a.body.requestId`; `@after(trace) c` | `b` *error* (missing value); `trace` and `c` *blocked* | `b` error; **`trace` runs** (its value exists); `c` runs |
| M3 | M2 under `on_error stop` | the failed check stops the run; `b`, `trace`, `c` *skipped* | the same |
| M4 | A step errors inside a branch of an `if`; a second step of the branch; a step after the `if`; a second `if`; a last step | everything after the error *blocked*, five steps | each runs unless it reads a value that does not exist |
| M5 | Cleanup written as a chain: `tidy`; `@after(tidy) tidy2` reads a value that never came into being; `@after(tidy2) tidy3` reads nothing | `tidy2` *skipped*, and **`tidy3` skipped with it** | `tidy2` skipped; **`tidy3` runs** |

M5 is a defect that FLOW-00 would have made common: its rule 2 chains the
steps of a cleanup, and today one cleanup step without its input takes the
rest of the cleanup with it. Today's cleanups mostly carry no `@after`, so
every statement runs on its own.

### The rules

1. **A step is finished** when it passed, failed, errored or was blocked.
   The next step of the chain waits for that, not for success.
2. **`on_error stop`** (the default): the first failed check, error or
   blocked step stops the main flow, as today. Nothing changes.
3. **`on_error continue`**: the next step starts when its predecessor has
   finished and **every value it reads exists**. A value exists when the
   statement that makes it produced it, whatever that statement's result: a
   request that got an answer has its `status`, `headers` and `body` even at
   500.
4. A step that reads a value which does not exist is **blocked** and names
   the statement that should have made it. *Blocked* no longer travels along
   the chain; it travels only through values.
5. A request that got an answer without the field read (`a.body.id` at 500)
   errors with *Missing value*, as today; the step after it runs.
6. **An `if`** that could not decide (its condition reads a value that does
   not exist) is blocked; the steps in both branches are blocked by it; the
   step after the `if` runs. A branch that was not taken is *skipped*, which
   is not a failure and blocks nothing (LANG-14).
7. **A component call** that fails inside finishes as *error*; an export it
   had already computed exists, the others do not. The cleanup already reads
   it this way (`engine.rs`: "exported resource IDs remain usable for
   cleanup"); the main flow does the same under `continue`.
8. **The cleanup** runs whatever happened, as today. A cleanup step whose
   value does not exist is skipped, as today, and **the cleanup steps after
   it run** (M5).
9. A run with a failed, errored or blocked step is *failed*, as today.

The three kinds of dependence a reviewer asked to be kept apart are all
here without syntax: order (the chain), success (`on_error stop`), data
(the values a step reads). The engine already tells an order edge from a
data edge (`data_dependency` in `engine.rs`) and already judges single
values in the cleanup (`is_available`); the change moves that gate into the
main flow.

What the owner should know: under `continue` a step that reads nothing from
the failed one **now runs** — `ship` after a failed `pay`, unless `ship`
reads the payment. That is what `continue` says; a scenario that must not
go on uses `stop` or an `if`.

## 2. The language version (LANG-15)

Today: the application is 0.1.0, the folder manifest has `formatVersion: 1`
(ADR 0010), the legacy workspace `FORMAT_VERSION = 2`, the reference says
"Reference version: 1". Nothing names the language.

Measured with the released 0.1.0 binary: the manifest is read strictly
(`deny_unknown_fields`, version first).

| Manifest | 0.1.0 says | Opens? |
| --- | --- | --- |
| `formatVersion: 1` and an unknown `languageVersion` | *Invalid folder project metadata; existing data was preserved.* | no |
| `formatVersion: 2` | *Unsupported folder format version; existing data was preserved.* | no |

Either way 0.1.0 reads nothing wrongly. Only the second message is true,
and adding a field **is** a change of the manifest's format.

Proposed rules:

1. `languageVersion` is a whole number in the manifest, raised only when a
   file that was valid means something else or becomes invalid. An addition
   an older build refuses at its line (`uuid()`, LANG-10) does not raise it.
2. A manifest without the field is language 1. The manifest that carries the
   field is `formatVersion: 2`; from then on the two numbers move apart.
3. A build reads **one** language version, its own.
   - Project newer than the build: not opened for running or editing; the
     window, the command line and the MCP server say *This project is
     written in language 3; this build reads language 2. Update API Forge.*
   - Project older than the build: opens for reading and editing; `check`
     says so at the manifest and names the fix; **nothing runs** until the
     fix has passed. The build does not run language 1 with its old meaning
     and does not quietly run it with the new one.
4. The fix raises the number as its last write, when every source passes
   the check. With an error left, the number stays and the report lists
   what the person places by hand.
5. Language 2 is: the rules of FLOW-00 (as amended by section 4), RUN-03
   and section 1. The reference's first line and `project_overview` name the
   number.
6. The legacy JSON workspace has no language version and stays outside
   (FLOW-00, consequence 2). A lone file (`apiforge fmt file.afs`) is read
   in the build's language.

## 3. Edit, fix, format: three operations on a file's text

Verified in the code: every statement, attribute and expression of the
syntax tree carries its byte range; a diagnostic may carry a `Fix { range,
replacement }`, already applied by the code view and served to an agent
(`ast.rs`, `folder/code_view.rs`, `apiforge-mcp/src/tools.rs`); a comment
above or beside a statement is kept as its note, a free comment is dropped
by a reprint after the comment-loss question (`comments.rs`); a board edit
and a publication print the whole document from the model (`print.rs`,
`publish.rs`); `print` and `fmt` share one writer and differ only in the
attribute order `print` builds (`@label` first).

| Operation | Started by | May change | Mechanism | Ticket |
| --- | --- | --- | --- | --- |
| **Edit** | a gesture on the board, an MCP edit | the statement edited, and a statement moved or inserted by it | replace or insert the statement's range; the rest of the text is not read back from the model | LAY-34 (#396), owner's decision A |
| **Fix** | the person: a click on an error, *fix the file*, `apiforge fix` for a project | only what the new language requires | the diagnostics' `Fix` ranges, applied together | LANG-13 (#386) |
| **Format** | the person only: *Format* in the editor, `apiforge fmt` | the whole file's layout, never its meaning | the one writer | LANG-06 (#143), KTLO-37 (#211), then EDIT-03 (#369) |

For all three: a comment the person wrote stays; the same operation applied
twice changes nothing the second time; the window, the command line and the
MCP server do the same thing because they call the same function.

- **Nothing formats as a side effect.** Saving, a board edit and a fix
  never reformat. That closes LAY-34's report and makes EDIT-03 safe: new
  layout rules reach a file only when its author asks.
- **Format keeps free comments** where they stand between statements
  (LANG-06): it prints statements in the order written and has the comment
  tokens with their ranges. Only printing from the model has no place for
  them, and after LAY-34 that happens for a document without text, for
  `print`, and at publication.
- **One attribute order** (KTLO-37): the order the writer uses is the
  canonical one, Format normalises to it, an edit leaves the author's order
  alone, and the reference shows it. Which order is decided with section 4:
  without `@after` only `@label`, `@contract` and `@note` are left.
- **The fix is small on purpose.** It writes each file whole or not at all
  and raises the version last. No transaction over the project, no undo
  beyond the editor's and Git: cut, by the owner's frame.

Not verified: whether publication can keep the author's text instead of
printing from the model (it decides whether a free comment reaches a
revision). Left to LANG-06.

## 4. `@after` against the order of the lines

One round, as the owner asked. The finding that prompts it: FLOW-00's rules
3 and 4 leave `@after` nothing to say that the order of the steps in a
block does not say. Before FLOW-00 an `@after` could fan out, join and name
a check; after it, each step names exactly the step above it.

- **A — explicit `@after`** (FLOW-00 as approved): every step names its
  predecessor; the order of lines means nothing.
- **B — the order written**: the steps of a block — the scenario, a branch,
  the cleanup, a component — run in the order they are written. `@after`
  leaves the language. Checks and values still stand anywhere and run when
  what they read exists (RUN-03); they compute nothing but a result, so
  their place cannot change a run.
- **C — hybrid**: the order written, with `@after` kept for a step written
  elsewhere.

### The examples

Requests are shortened to one line. Lines marked `▸` exist only in A.

**E1. A linear flow with checks.**

```afs
scenario "Buy coffee" {
  begin = start
▸ @after(begin)
  login = POST "/login" { … }
  logged_in = assert login.status == 200
▸ @after(login)
  cart = POST "/carts" { … }
  created = assert cart.status == 201
▸ @after(cart)
  paid = POST "/carts/{{cart.body.id}}/pay" { … }
▸ @after(paid)
  receipt = GET "/receipts/{{paid.body.receiptId}}"
}
```

**E2. Adding a step in the middle** (a coupon before paying).

```diff
 A                                       B
+@after(cart)                           +coupon = POST "/carts/{{cart.body.id}}/coupon" { … }
+coupon = POST ".../coupon" { … }
-@after(cart)
+@after(coupon)
 paid = POST ".../pay" { … }             paid = POST ".../pay" { … }
```

In A the second edit is on another statement; forgetting it gives "`cart`
is followed by two steps". Removing `coupon` again is two edits as well.

**E3. A branch, and going on after it.**

```afs
▸ @after(cart)
  paid = if cart.body.total > 0 {
    payment = POST "/payments" { … }
▸   @after(payment)
    receipt = GET "/receipts/{{payment.body.id}}"
  } else {
    free = POST "/carts/{{cart.body.id}}/confirm"
  }
▸ @after(paid)
  logout = POST "/logout"
```

**E4. A branch inside a branch.**

```afs
▸ @after(order)
  shipped = if order.body.items.length > 0 {
    label = POST "/labels" { … }
▸   @after(label)
    abroad = if order.body.country != "PL" {
      customs = POST "/customs" { … }
▸     @after(customs)
      duty = GET "/customs/{{customs.body.id}}/duty"
    }
▸   @after(abroad)
    dispatch = POST "/dispatch" { … }
  }
▸ @after(shipped)
  mail = POST "/mail" { … }
```

**E5. Moving a step** (`address` before `coupon` instead of after). A: three
`@after` change and no line moves, so the file no longer reads in the order
it runs — and after LAY-34 nothing puts it back. B: the statement's lines
move; the file cannot disagree with the run.

**E6. The cleanup.**

```afs
  cleanup {
    drop_items = DELETE "/carts/{{cart.body.id}}/items"
▸   @after(drop_items)
    drop_cart = DELETE "/carts/{{cart.body.id}}"
▸   @after(drop_cart)
    drop_user = DELETE "/users/{{user.body.id}}"
  }
```

**E7. A loop** — the shape only; its words are part B's (LANG-16).

```afs
▸ @after(cart)
  emptied = for item in cart.body.items {
    removed = DELETE "/carts/{{cart.body.id}}/items/{{item.id}}"
    gone = assert removed.status == 204
▸   @after(removed)
    stock = GET "/stock/{{item.productId}}"
  }
▸ @after(emptied)
  empty = GET "/carts/{{cart.body.id}}"
```

**E8. Waiting for a state** — the shape only.

```afs
▸ @after(order)
  moved = POST "/orders/{{order.body.id}}/transition" { body { to: "packed" } }
▸ @after(moved)
  packed = retry until state.body.status == "packed" within 5s every 200ms {
    state = GET "/orders/{{order.body.id}}"
  }
▸ @after(packed)
  ship = POST "/orders/{{order.body.id}}/ship"
```

### Measured on the repository

In the twelve `.afs` files, none of the 52 `@after` names a statement
written below it: every file is already written in the order it runs.
(Counted by script; a step ordered only by a value written below it was not
counted.) `shop-30` has 30 steps and 5 `@after`, all to a check. Under A
the fix adds about 25 lines to it and rewrites the 5; under B it deletes
the 5.

### Judged

| Criterion | A — `@after` | B — the order written | C — hybrid |
| --- | --- | --- | --- |
| Reading, for a tester | Same text plus one line a step; the chain is read off names | Top to bottom, as in every test tool | As B, until a file uses `@after` |
| Amount and repetition | One attribute per step, each repeating the name above it | None | Little |
| Editing by hand or by an agent | Insert, remove and move each touch a second statement; a rename touches the successor | One place | Two spellings to know |
| One meaning | Yes, but the lines may stand in another order than the run (E5) | Yes: one rule per block; a step cannot be written and not run | Two ways to write one flow |
| Board and round trip | Any graph prints, also a broken one, with errors | Only a chain prints: the board must always place a step (DIAG-06 becomes a condition, not a comfort) | As A |
| Errors left to explain | no `@after`; two successors; `@after` on or to a check; two names; a step outside the flow | `@after` is gone (with a fix) | all of A |
| Cost | LANG-13 M + LANG-12 S–M as estimated; nothing reversed | LANG-13 M, LANG-12 not needed; lowering adds the edge from the step above, the writer drops the attribute; the generated-graph round-trip test is limited to chains | A plus B |
| Decisions touched | none | FLOW-00 decisions 2 and 8 and consequence 1 are replaced; 1, 3–7 stand; ADR 0005's "line order means nothing" is amended (FLOW-00 amends it anyway) | — |

### Recommendation: B

Not because it is shorter. Once a step may follow only one step and be
followed by one, `@after(previous)` and the order of the lines carry the
same fact twice, and two copies of a fact can disagree: E2 and E5 are that
disagreement, as an error in one case and as a file that reads in the wrong
order in the other. FLOW-00's own goal — "the order can be read top to
bottom in the code" — holds in A only while something reprints the file,
and LAY-34 removes that. B makes it hold by construction, removes four
kinds of error instead of explaining them, and is what part B's blocks need
inside their braces (E7, E8).

What B costs, plainly:

- Two decisions the owner took today are replaced (2: `@after` mandatory;
  8: `@after` on steps only), and consequence 1 (`@after` takes one name)
  falls away. Decision 6 of the LANG quiz holds better than before: the fix
  **deletes** `@after` lines and touches nothing else, when the file is
  written in the order it runs — every repository file is.
- LANG-14 keeps what it does at run time (a step after an `if` waits for
  the branch taken) and its braces rule; its spelling `@after(x)` would live
  for one sprint. LANG-14 is not blocked; if B is chosen before it merges,
  it skips the spelling.
- A step can no longer be on the board and outside the flow. DIAG-06's
  mock-up shows where a dropped step lands.
- A statement that runs nowhere today (F2 of FLOW-00) would start running
  if merely kept; the fix refuses such a file and names the statement.
- A published revision is not rewritten (FLOW-00, consequence 2): for a
  stored revision the lowering keeps reading `@after`, the one place the old
  spelling survives. A revision is printed from the model, in run order, so
  one without `@after` already runs in the order written.

C is rejected in both cases: two spellings of one flow is what an agent and
a formatter handle worst.

If the owner keeps A, it needs two things it lacks today: Format sorts the
steps into chain order, and `check` warns when they are not (E5).

## Owner decisions needed

Recommended option first.

1. **How a chain is written** (section 4).
   **A** the order written: `@after` leaves the language; LANG-12 is not
   needed. **B** `@after` stays as FLOW-00 approved, with Format sorting
   the steps and a warning when the lines disagree with the run.
2. **How the manifest carries the language version** (section 2).
   **A** `formatVersion: 2` with `languageVersion`: 0.1.0 answers
   *Unsupported folder format version*. **B** the field alone under
   `formatVersion: 1`: 0.1.0 answers *Invalid folder project metadata*, as
   for a damaged file.
3. **A project in the older language, opened by a newer build** (section 2).
   **A** opens, is checked and edited, runs only after the fix. **B** runs
   at once with the new meaning, with a notice.

## To confirm at approval

1. Under `on_error continue` an export of a component call that failed
   later is readable in the main flow, as it is in the cleanup today
   (section 1, rule 7).
2. A request that answered without the field read stays an *error* at the
   reading step, not a *blocked* (rule 5).
3. Cut from the 0.2.0 conditions, by the owner's frame: a reversible or
   transactional migration, and tests on project files gathered from 0.1.0
   users. Kept: the fix on every repository scenario and the owner's test
   projects with the order of steps compared before and after; comments
   kept by the fix (it costs nothing: the fix edits ranges).
4. The writer's attribute order is the canonical one (KTLO-37); the
   reference follows it.

## Tickets

| Order | Ticket | Delivers | Estimate |
| --- | --- | --- | --- |
| 1 | **RUN-01** #126 | Section 1 in the runner and in the reference | M · 20 min · `default` |
| 2 | **LANG-15** #412 | Section 2: the manifest, the three refusals, the reference's first line | S–M · 20 min · `format` |
| 3 | **LANG-13** #386 | The chain as decided in 1, with the fix; `apiforge fix`; the repository's files | M · 30 min · `format` |
| 4 | **LANG-12** #385 | Only if `@after` stays | S–M · 20 min · `format` |
| 5 | **DIAG-06** #391 | The board keeps the chain; mock-up first | from the mock-up |
| — | **LANG-06** #143 with **KTLO-37** #211 | Format keeps free comments; one attribute order | S · 15 min · `format` |

The order of the whole sprint, the files each ticket touches and what may
run side by side are in [the sprint plan](../sprints/lang.md).

### Acceptance criteria — RUN-01

All under `on_error continue` unless said, simulated and live.

1. `a` errors without an answer; `b` follows and reads `a`; `c` follows `b`
   and reads nothing: `b` is blocked and names `a`; `c` runs.
2. `a` answers 500 with a body; a step that reads a field the body has
   runs; a step that reads a field it lacks errors; the step after either
   runs.
3. A check that reads a value which does not exist is blocked; a check on a
   later step that ran passes or fails on its own.
4. An error inside a branch: the next step of the branch and the step after
   the `if` each run unless they read a missing value. An `if` that cannot
   decide is blocked, both branches are blocked by it, the step after it
   runs.
5. A component call that fails after computing an export: a step reading
   that export runs; a step reading another export is blocked.
6. In a cleanup of three chained steps where the second lacks its value,
   the first and third run and the second is skipped.
7. Under `on_error stop` every run of the repository's run corpus is
   unchanged.
8. The run is *failed* in 1–6; the report names, for each blocked step, the
   statement whose value was missing.

## Not in scope

- Loops, block results, scopes, a value from whichever branch ran, and the
  picture of a block: LANG-16 (#413), then LANG-08, LANG-07, LANG-09.
- A way to keep a step in the file without running it: asked in #386, the
  same question in A and B; analysed with LANG-13.
- Blank lines and long bodies in the formatter: EDIT-03 (#369), after the
  release.

## Technical appendix

- Section 1: `apiforge-runner/src/engine.rs`, the gate (`blocked`,
  `inactive`, `is_available`) — the cleanup's rule for a single value moves
  to the main flow under `continue`; `apiforge-core/src/run.rs`
  (`is_unavailable`); `fixtures/runs/error-policy-continue.json`. Same file
  as RUN-03 (#409): RUN-01 starts when that is merged.
- Section 2: `apiforge-storage/src/project/schema.rs` (`ProjectManifest`,
  the version read first), `ProjectError::UnsupportedVersion`; the
  reference's first line is tied by `apiforge-project/tests/reference.rs`
  and `apiforge-mcp/src/reference.rs`.
- Section 3: `apiforge-script` `ast.rs` (`Fix`, ranges), `parser.rs`
  (`attach_comments`), `format.rs` (the writer), `print.rs` (it builds
  `@label` before `@after`); `apiforge-project` `comments.rs`,
  `publish.rs`.
- Section 4, B: `lower.rs` adds the ordering edge from the previous step of
  the same braces; `print.rs` writes steps in chain order (it already
  orders topologically) and no `@after`; `validation.rs` loses the checks
  LANG-12 would add. The experiments of section 1 and the counting script
  were run on copies of `fixtures/workspace.json` outside the repository.
