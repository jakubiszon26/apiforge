# REQ-06: Acceptance of the REQ sprint — the owner's own run, and one mandatory scenario

- Size: S · Owner review: 10 min, plus the owner's own run · Risk: none
- State: **Analysis; questions 1–4 wait for the owner** · the mandatory
  scenario was added by the owner on 2026-11-21 (#463) · REQ sprint, last
- Analysis model: Fable 5.2 · Implementation model: — (the owner at the
  window; before him, agents under [A/B rounds](../process/ab-testing.md))

## Goal

The owner does on the finished build what the run of 2026-11-19 could not
do comfortably, and says whether the sprint is done. Since 2026-11-21 one
scenario is mandatory: **POST → take the ID from the response → GET with
that ID → an assertion → DELETE**, everything from the interface, with no
hand editing of the `.afs` file. "If after REQ this cannot be done
comfortably, in my view the sprint has not fully solved the functional
tester's problem." The scenario is also run by agents, and the most
ergonomic interface for it is worked out by A/B tests.

## The task

**Part 1 — the mandatory scenario.** An empty project, the demo API on
loopback (`apiforge-demo-api`: `POST /demo/shop/carts`,
`GET` and `DELETE /demo/shop/carts/{id}`), window 1280 × 800, no stored
settings. The instruction, and nothing else:

> Write a test that creates a cart with two of product 123, reads the cart
> back by the id the API gave it, checks that it is the same cart with the
> same quantity, and deletes it. Run it twice.

**Part 2 — on `labels` (FIX-01), as written on 2026-11-19**, without
opening Code:

1. Read from the list which requests the scenario sends and what each
   checks.
2. Change the weight sent by *Weight 31 kg*, run to that step, and read
   the response beside the request.
3. Make one check fail (change an expected value), run, and find in Run
   results which check of which request failed and what came back, without
   opening a row.
4. Add an authorization to *No token* and remove it again; the `.afs` diff
   holds only those edits.
5. Send one request alone.

## Acceptance criteria

1. **The mandatory scenario is done in the window.** Code is not opened
   and no file is edited outside the application. The result has the POST
   with its body, the GET whose path reads the id of the POST's response,
   a check on the GET's response that compares with what was created (the
   id and the quantity), and the DELETE with the same id.
2. **Nothing of the language is typed by heart.** The id reaches the GET
   and the DELETE by choosing it, not by typing `{{create.body.id}}`; the
   check is made from the response, not written.
3. **It passes twice.** The second run, with nothing changed, is green:
   the API gives a new id each time, so a check that froze the first id
   fails here.
4. **The DELETE** stands where [question 1](#questions-for-the-owner)
   decides, and after both runs the API holds no cart of the test.
5. **The file says what was done:** the saved `.afs` holds the three
   requests and the checks, and no line the tester did not ask for beyond
   what [question 3](#questions-for-the-owner) allows.
6. **Agents first.** Before the owner's run a baseline round is run on the
   release build ([A/B rounds](../process/ab-testing.md)): the shortest
   path and three first-time runs, recorded under
   `docs/qa/<date>/REQ-06/`. The bar it has to meet is
   [question 2](#questions-for-the-owner).
7. The owner does part 1 and part 2 on a release build of `main` and
   records the result on #463: done, or the findings.
8. Each finding becomes an issue or is fixed before the sprint closes.

## What the owner will see

Nothing new from this ticket.

## Mock-up

Not applicable: no interface change.

## The scenario against v0.2.0 and the plan

Walked by the planning model on the released v0.2.0 package (Linux, an
isolated session at 1280 × 800, live against the demo API) on 2026-11-21:
one run, not a counted round. The whole scenario **can be built in the
window today**; it cannot be built without knowing the language, and its
natural check is wrong.

| Step | v0.2.0 | Planned ticket | Gap |
| --- | --- | --- | --- |
| A new scenario and its first request | Explorer **+** → *New scenario* → it opens in **Code**; *Diagram* → *Add an HTTP request* | REQ-01 lists steps; it does not say a step can be added there | **G1** adding a step from *Steps* |
| POST with a body | Inspector: method, URL; the body is below the fold of *Configuration* | REQ-02a, REQ-02b | — |
| Send it and see the response | The whole scenario, after choosing *Live* in the run-mode menu | REQ-04, REQ-05; UX-42 for the mode | **G2** REQ-05 has no rule for a request that reads nothing and has never run — the first request of every new test |
| Take the id from the response | Typed by hand: `{{create.body.id}}`. Completion offers the steps, and **no fields** — before a run and after one ([capture](../qa/2026-11-21/REQ-06/baseline-v0.2.0-id-typed-by-hand.png)) | REQ-02b: "completion of parameters, steps and their fields" | **G3** fields come from a sample only, and nothing takes a value *from the response* to a later request |
| GET with the id in its path | Works once typed; the board draws the data line and names the source | REQ-02a (the URL field) | **G4** REQ-02b draws placeholders as chips in the Body only |
| An assertion on the GET | Run results → *Check* on `id`: one click, and it writes `read.body.id == "498c383b-…"`, the id of that run ([capture](../qa/2026-11-21/REQ-06/baseline-v0.2.0-check-is-a-literal.png)). The second run fails ([capture](../qa/2026-11-21/REQ-06/baseline-v0.2.0-second-run-fails.png)). Before a run the ✓ of *Output data* did nothing (#399, fixed on `main` by DIAG-09) | DIAG-08 adds the type check; REQ-10 marks values that changed between two runs, in *Expect this response* only | **G5** a check from a field can only compare with the value of this run |
| DELETE with the id | *Cleanup* tab → *Add an HTTP request* → DELETE, the path typed as above; it runs after the test and reads the POST's id | REQ-01 does not say where Cleanup is in *Steps*; REQ-04 keeps Cleanup running | **G6** Cleanup in the list of steps |
| Run twice, read the file | Passed, then Failed (G5). The file has what was built and also `@config({ method: "POST", timeoutMs: 10000.0 })`, `timeout 10s` on each request, `@label("Check id")`, `begin = start` ([file](../qa/2026-11-21/REQ-06/baseline-v0.2.0-cart-lifecycle.afs)) | — | **G7** the window writes lines nobody asked for |

The run in numbers: 30 pointer actions, 8 text entries (about 210
characters, 36 of them language syntax), 2 scrolls of the Inspector,
2 wrong turns (the *Checks* tab has no way to add a check; the ✓ before a
run), 1 wrong result, Code not opened. Not tried: repairing the check in
the Inspector so that it reads `create.body.id`; the base URL; a
simulation with samples.

**The demo API is enough**: carts are created, read and deleted by id,
the id is new on every create, a deleted cart answers 404. Nothing is
needed in the fixture.

### What each gap becomes

Nothing below is scheduled; each waits for
[question 4](#questions-for-the-owner).

| Gap | Proposal | Form |
| --- | --- | --- |
| G3 | **REQ-11: a value of a response is used in a later request by choosing it** — from the field in the response (*Use in…*), and from the field being filled (a list of the earlier steps' fields, fed by the last run as well as by the sample) | New ticket, [#538](https://github.com/jakubiszon26/apiforge/issues/538), proposed. M · `visible UI`. Which gesture: an A/B round |
| G5 | **REQ-12: a check made from a field offers what to compare with** — this value, the value an earlier step gave or sent when it is equal now (`create.body.id`, the quantity sent), or the type (DIAG-08) | New ticket, [#539](https://github.com/jakubiszon26/apiforge/issues/539), proposed. S–M · `visible UI`, `default` (what one click writes). After DIAG-08 (#521) and the redesign of *Output data*, which own the place; this ticket does not change them |
| G1, G6 | Two questions added to REQ-00: a step is added from *Steps* after the selected one; Cleanup is a section of the list after Main and a step can be put there | Criteria of REQ-00 and REQ-01 |
| G2 | REQ-05: a request that reads nothing is sent alone with no last run; and a single send of a request that creates something says that nothing cleans it up | Criteria of REQ-05, with its five open questions |
| G4 | REQ-02b: placeholders are chips with completion in the URL, the query and the headers as in the Body | Criterion of REQ-02b |
| G7 | What the window writes for a step nobody configured (`timeout 10s`, `@config`, a `@label` equal to the handle) | Proposal, below; a default, so the owner's |

REQ-02a (#458) and DIAG-08 (#521) are being changed by the redesign of
the Inspector's *Output data* tab (DIAG-10 to DIAG-14, #531–#535) and are
not edited here. REQ-11 and
REQ-12 both put an action on a field of a response, so their place is
decided with that redesign, not beside it.

## A/B rounds in the REQ order

The method is [A/B rounds](../process/ab-testing.md). REQ-00's views are
decided (5 = A, 6 = A) and its captures are drawings, which cannot be
driven; no round is proposed for the shape. Rounds stand where this
scenario has real alternatives:

| When | Round | Variants | Gates |
| --- | --- | --- | --- |
| Now | **AB-01**, v0.1.0 against v0.2.0; this scenario is its task T2 and gives the counted baseline | the two releases | nothing; [AB-01](AB-01.md) |
| REQ-11's analysis | A value from a response into a later request | Today · A completion fed by the last run · B *Use in…* on the response field · C dragging the field onto the request's field | REQ-11 |
| REQ-12's analysis | A check from a field | Today (literal) · DIAG-08 as built · A the offer of sources in the ✓ menu · B the source chosen in the new check's row | REQ-12 |
| After REQ-05 merges | Baseline of the request view: change a body, see the response | *Run to here* · *Send alone* | nothing; findings before REQ-06 |
| REQ-06 | Baseline of the whole scenario on the release build | one | the owner's run |

## Questions for the owner

Recommended first.

1. **Where is the DELETE in the mandatory scenario?**
   A. Both: a step of the test with its own check, then the scenario once
   more with it moved to Cleanup — tests the endpoint and the tidy-up.
   B. In Cleanup only — it runs even when a check fails; the DELETE itself
   is not asserted.
   C. A step of the test only — as the sentence reads; a failed check
   leaves the cart behind.
2. **What does "comfortably" mean, as the bar the agents' round must
   meet before your run?**
   A. No language syntax typed, green twice, no wrong turn in at least two
   of three first-time runs, and fewer actions than v0.2.0 — your own run
   is still the verdict.
   B. Only: done in the window and green twice — the rest is reported, not
   required.
   C. A fixed number: the shortest path within 20 pointer actions.
3. **The mandatory scenario runs against…**
   A. The demo API, live — a real server, a new id every run.
   B. Live, and then the same scenario as a simulation from saved samples
   (needs REQ-10).
   C. A simulation only — nothing is sent; the id never changes, so the
   wrong check is not caught.
4. **The gaps.**
   A. REQ-11 and REQ-12 join the REQ sprint, and the criteria for G1, G2,
   G4 and G6 are added to REQ-00, REQ-01, REQ-02b and REQ-05 — the
   scenario cannot meet criterion 2 or 3 without them.
   B. REQ-11 joins; the check (REQ-12) goes to the sprint after — the
   scenario is accepted with the check repaired in the Inspector.
   C. Criteria only, no new ticket — the id is typed with completion and
   criterion 2 is relaxed.
5. **How many runs per variant in an A/B round?**
   A. The shortest path and three first-time runs — shows a dead end
   every driver meets; about an hour of agents per round.
   B. The shortest path and one first-time run — half the cost; one
   driver's accident reads as a finding.
   C. The shortest path and five — steadier; a round takes an afternoon.
6. **Which tickets get a round?**
   A. Only those the sprint's plan names, proposed by the planning model
   and confirmed by you with the plan.
   B. Every `visible UI` ticket of size M or above.
   C. Every `visible UI` ticket.
7. **Which features first?** (the list is in
   [Proposals](#proposals-beyond-the-request))
   A. The three recommended: AB-01, the value from a response (REQ-11),
   the check from a field (REQ-12 with DIAG-08).
   B. Those and the first start (DEMO-01) and inserting and moving a step
   (DIAG-06b).
   C. AB-01 only for now; the rest after its results.
8. **The budget of a round.**
   A. 90 minutes of agents and 15 of yours; a larger round is split.
   B. 45 and 10 — two variants and two tasks at most.
   C. No cap; the plan states the cost of each round.
9. **AB-01, v0.1.0 against v0.2.0: when?**
   A. Now, finished before `v0.2.1-rc.1` is tagged — a regression that
   blocks a task can still go into 0.2.1; the rest goes to REQ.
   B. Right after 0.2.1, comparing v0.1.0 with v0.2.1 — nothing delays the
   patch; regressions wait a release.
   C. Both: now, and the failed tasks again on `v0.2.1-rc.1`.

## Not in scope

- New behaviour; fixing what the runs find (a ticket each).
- REQ-02a (#458) and DIAG-08 (#521): changed by the redesign of *Output
  data*; this ticket names its dependencies on them and edits neither.

## Proposals beyond the request

Features worth a round, ranked. Cost is agents' time · the owner's
minutes; building variants is an Opus session on a throwaway branch and
is counted with the ticket's analysis.

| # | Feature | Task for the drivers | Variants | Besides the standard measures | Cost | Gate or after |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **v0.1.0 against v0.2.0** ([AB-01](AB-01.md)) | Eight tester's tasks | the two releases | regressions by class | 4 h in two rounds · 20 | a ticket of its own; decided |
| 2 | **A value from a response into a later request** (REQ-11; touches the *Output data* redesign) | Part 1 of this ticket up to the GET | Today · completion · *Use in…* · drag | syntax typed; the wrong field chosen | 90 min · 15 | gates REQ-11 |
| 3 | **A check from a field** (DIAG-08's ✓ menu, REQ-12, *Check* in Run results) | "Check the cart read back is the one created, and that `total` is a number"; run twice | Today · DIAG-08 as built · sources in the menu · source in the row | wrong result on the second run | 90 min · 15 | after DIAG-08 (approved for 0.2.1, not delayed); gates REQ-12 |
| 4 | The first start (DEMO-01) | "You have just installed this. Say what the test checks, then make it fail on purpose." First-time runs only — a fresh driver is the newcomer | the 0.2.0 demo · DEMO-01's | right answers; where the driver looked first | 45 min · 10 | after DEMO-01 merges, before `v0.2.1` |
| 5 | Inserting and moving a step (DIAG-06b; REQ-00 question 6) | "Put a request between steps 2 and 3; make step 5 run before step 4" | drag onto the line · **+** on the line · *Move earlier / later* · dragging in *Steps* | steps left out of order; the diff | 90 min · 15 | after DIAG-06b; gates REQ-00's question 6 |
| 6 | Reading a scenario: *Steps* against *Canvas* (REQ-01) | Six questions with known answers on `labels` and *Room booking day* | Canvas · Steps | right answers | 60 min · 10 | after REQ-01 merges; findings before REQ-02a |
| 7 | *Run to here* against *Send alone* (REQ-04, REQ-05) | Change a body value and read the new response | the two buttons | requests sent that were not needed | 45 min · 10 | after REQ-05 |
| 8 | Headers and authorization (REQ-03) | Add a bearer token to one request and remove it | Today · the table and *Auth* | the diff | 30 min · 5 | after REQ-03 |
| 9 | The run-mode switch (UX-42) | Run once as a simulation and once live | the menu · the switch | — | 20 min · 5 | not alone: decided on the board; part of a v0.2.0 against v0.2.1 round if one is run |

Recommended first: 1, 2 and 3. Not proposed: pasting cURL (REQ-07) and
duplicating a case (REQ-08) — each replaces typing with one action, and a
round would only confirm it.

Other proposals:

| Proposal | Problem it removes | Size · trade-off | Goes to |
| --- | --- | --- | --- |
| **The window writes only what was set** (G7) | A request added in the window is saved with `timeout 10s` and, for a POST, `@config({ method: "POST", timeoutMs: 10000.0 })`; a check gets a `@label` that repeats its handle. The diff of a test is longer than the test, and criterion 5 cannot be met | S–M · `default`: what a new step contains. A project saved by 0.2.0 keeps its lines | REQ, with REQ-02a's "writes the same `.afs`"; or LANG's second half |
| **A new scenario opens where it will be built** | *New scenario* opens Code with two lines; a tester who works in the window turns to Diagram first, every time | S · `default` | REQ-01, as a criterion once *Steps* exists: which of Canvas, Steps and Code a new, empty scenario opens in |
| **A regression task set kept as a fixture** | AB-01's eight tasks, their start projects and pass conditions are written once and cost nothing to run again on every release candidate | S; the sheets must be kept true as the interface changes | after AB-01, a line of REL tickets: "the task set was run on the candidate" |
| **The pass condition as a command** | A driver's "done" is checked by hand today. `apiforge check` and `apiforge run --json` already say most of it; a small script per task makes a round's verdict mechanical | S, tooling only | with AB-01 |

## Technical appendix

- Demo API: `crates/apiforge-demo-api/src/lib.rs` — `create_cart` answers
  201 with `id`, `productId`, `quantity`, `total`, `createdAt`;
  `delete_cart` answers 200 `{ "deleted": true }`, then 404.
- Completion in the URL and the body lists steps from the document and
  fields from `inspector::response::sample_output`; a run's result is not
  a source (G3). *Check* in Run results writes a comparison with the
  field's value (`report.rs`); *To variable* exists beside it.
- The baseline was driven through the `kwin` MCP server by screenshots;
  input sent before the window had drawn was lost once, which is why the
  process forbids blind input.
