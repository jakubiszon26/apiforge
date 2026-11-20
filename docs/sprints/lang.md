# LANG: the language sprint — plan

- State: **proposed 2026-11-16**. The order below is the owner's (LANG
  quiz of 2026-11-16, recorded on
  [#388](https://github.com/jakubiszon26/apiforge/issues/388)), and the
  three decisions of [LANG-00](../tickets/LANG-00.md) are taken: a chain is
  written by the order of its lines, the manifest carries the language
  version under `formatVersion: 2`, a project in the older language runs
  only after the fix.
- **Started by the owner on 2026-11-20.** It had been paused since
  2026-11-17 until LAY closed; LAY closed on 2026-11-19 (#296) and the
  owner confirmed the same day that part A, to 0.2.0, runs first and the
  [REQ sprint](req.md) after it. The start did not wait for the LAY
  [retrospective](../retrospectives/2026-11-20-lay.md), written the same
  day. The first half runs in the order below; which of its tickets may be
  taken unattended is on the issues (`ready:*`), not here. RUN-01, LANG-15,
  LANG-13 and FIX-01 (#454, a ticket of REQ whose scenario is a case for
  the fix) were taken on 2026-11-20, each on its own branch with its
  ticket file as the first commit; three of them merged the same day
  ([First half](#first-half--to-020)). What each works in, the rulings on
  an independent review of the ticket files and the order since are
  recorded on #411 ([the first plan](https://github.com/jakubiszon26/apiforge/issues/411#issuecomment-7025097154),
  [its revision](https://github.com/jakubiszon26/apiforge/issues/411#issuecomment-7033966949)).
- **Three proposals accepted into scope on 2026-11-20**, for the second
  half: LANG-17 (#470) a table of cases, or a request based on another;
  LANG-18 (#471) headers and authorization shared by a scenario; LANG-19
  (#472) a check without an invented name and one check for several
  fields. Accepted means analysed and planned, not a design approved: each
  is `format`, and its questions return to the owner with its analysis.
  They come from the reference scenario `labels` (FIX-01, #454), whose
  sixteen `@after` lines are also a case for LANG-13's fix.
- Analysis model: Fable 5.2.
- Inputs: [FLOW-00](../tickets/FLOW-00.md), [LANG-00](../tickets/LANG-00.md),
  [LAY section 12](lay.md#12-after-the-checkpoint-2026-11-16), every open
  LANG, RUN, EDIT and CLI issue on 2026-11-16, the code on `main` at
  `7cc0660`.

## Goal

A tester writes a scenario that reads in the order it runs, keeps running
sensibly after a failure, and — in the second half — can repeat, wait and
choose. A file is safe to edit by hand, on the board and by an agent, and a
build that cannot read a project says so.

The sprint has two halves with a release between them:

1. **The one-chain language, released as 0.2.0.** Errors in a chain, the
   language version, the chain itself, the board that keeps it.
2. **The language grows**, designed from what a tester cannot write today
   (LANG-16), not ticket by ticket.

Not the goal: compatibility with 0.1.0. Breaking changes are allowed in the
0.x series (the owner, 2026-11-16); the repository's files and the owner's
test projects are updated, and the version detects the rest.

## Beside LAY

What LAY needs from the language and the run, and nothing else:

| Ticket | State | Why LAY needs it | Files |
| --- | --- | --- | --- |
| LANG-10 #354 `uuid()` | merged (#402) | — | — |
| RUN-03 #389 checks before the next step | *now*: pull request #409 | LAY-19 and the badge numbers of LAY-07 read the run's order | `apiforge-runner/src/engine.rs`, `apiforge-core/src/graph.rs`, `fixtures/runs` |
| DIAG-07 #392 a step added from *Next* runs after that step | *now*: pull request #400 | ahead of part B (LAY section 12.2) | `apiforge-core/src/edit/nodes.rs` |
| LANG-14 #390 a step after a whole `if`; a chain stays in its braces | *now*, as approved | LAY-34 and LAY-13 read which statements stand in a branch's braces | `apiforge-script` `lower.rs`, `print.rs`; `apiforge-core` `validation.rs`; `reference.md` |
| LAY-34 #396 a board edit changes one statement | *now*: LAY part B, after LANG-14 | a ticket of LAY | `apiforge-script` `print.rs`, `lower.rs`, `ast.rs` |
| LANG-00 part A #411 | approved; its tickets wait | — | `docs/` |
| DIAG-06 #391 and its mock-up | **paused** with the sprint | not needed by LAY | — |

LANG-14 goes ahead as approved. Its spelling `@after(x)` is the only way to
write the step until LANG-13, which keeps the lowering and removes the
spelling with every other `@after`.

## First half — to 0.2.0

In progress since 2026-11-20. **Merged that day:** FIX-01 (#485), RUN-01
(#487), LANG-15 (#486); beside them #474 and #476 (plans and process) and
#484 (CI). LANG-13 is pull request #488.

Merge order from here: LANG-13 → LANG-20, LANG-06, LANG-05 and LAY-45 side
by side → DIAG-06a → DIAG-06c → LANG-21 and DIAG-06b → REL-05.

| Order | Ticket | Size | Needs | Files |
| --- | --- | --- | --- | --- |
| done | RUN-01 #126 the chain goes on after an error; only a missing value blocks | M · 20 · `default` | merged in #487 | runner, ADR 0015 |
| done | LANG-15 #412 the language version in the manifest; the two refusals | M · 20 · `format` | merged in #486, with `LANGUAGE_VERSION = 1` | storage, project, command line, MCP |
| 1 | LANG-13 #386 the steps of a block run in the order written; `@after` leaves the language; the fix of one file; **the build reads language 2** | M, upper end · 30 · `format` | pull request #488; amended on 2026-11-20 (a component needs no `start`; a check that reads no step runs; the constant and the run refusal moved here from LANG-20) | `apiforge-script`, `apiforge-core` (`graph.rs`, `validation.rs`), `apiforge-project`, `apiforge-runner/src/plan.rs`, every test source with `@after`, `fixtures/scenarios`, `fixtures/scripts`, `reference.md`, ADR 0003 and 0005 |
| 2 | LANG-20 #481 `apiforge fix --folder` converts a whole project or nothing, in one commit of the store; `--check`, `--as-written`; the MCP tool `fix` | M · 15 · `format` | LANG-13 merged | `apiforge-cli`, `apiforge-mcp`, `apiforge-project/src/language.rs` |
| 2 | LANG-06 #143 with KTLO-37 #211 Format loses no comment, wherever it stands; one attribute order | M · 15 · `format` | LANG-13 merged | `apiforge-script` `parser.rs`, `ast.rs`, `format.rs`, `print.rs`, `edit.rs`; `reference.md` |
| 2 | LANG-05 #142 a check that is a comparison or a logical operation is written as typed; a bare value keeps `== true` | S · 10 · `format` | decided 2026-11-20 (A); LANG-13 merged; beside LANG-06, the second merges `main` first | `apiforge-script` `print.rs`, `format.rs`; `reference.md`, ADR 0005 |
| 2 | LAY-45 #468 the chain is drawn side to side, sockets on the cards' sides in both *Wires* modes; the badge arrow and the row-end mark go | M · 20 · `visible UI`, `removal` | moved here from LAY-2 by the owner on 2026-11-20; LANG-13 merged; brings `shop-30` with every wire back to 76 and 116 crossings | `apiforge-desktop/src/canvas/`, layout measurements |
| 3 | DIAG-06a #391 every edit keeps the chain: a wire to an existing step moves it, with an `if`'s braces and its checks, across braces, refused against a value read; *Add step* after the selected step; deleting an `if` asks once | M · 25 · `default`, `visible UI` | LANG-13, LANG-06 (`edit.rs`) and LAY-45 merged | `apiforge-core/src/edit`, `apiforge-script/src/edit.rs`, `apiforge-project/src/session.rs`, desktop |
| 4 | DIAG-06c #490 the board shows and takes the chain: the `if`'s own *Next*, the caption while carrying, a block lands on the nearest line, the row makes room | M · 20 · `visible UI`, `default` | LAY-45 and DIAG-06a merged | `apiforge-desktop/src/canvas/`, `dnd.rs`, `library.rs` |
| 5 | LANG-21 #482 the window offers *Update the project*: a check first, then *Update N files* or the list to place by hand | M · 25 · `visible UI` | decided 2026-11-20 (in 0.2.0; nine answers on the mock-up); LANG-20 merged | `apiforge-desktop/src/folder/`, `apiforge-project/src/language.rs` |
| 5 | DIAG-06b #491 a refusal shown while carrying; a notice after a delete; `+` on a line; *Move earlier* and *Move later* (Alt+↑, Alt+↓) | M · 15 · `visible UI` | DIAG-06c merged; not a condition of 0.2.0 | `apiforge-desktop/src/canvas/`, `folder/notices.rs` |
| 6 | REL-05 #414 release 0.2.0 | S · 15 | all above except DIAG-06b | release notes |
| — | LAY-13 #297 branch bands | — | after 0.2.0 (DIAG-06 question 6 = A) | — |

**Side by side.** Rows with the same order number touch different files:
LANG-20 (command line, MCP, `language.rs`), LANG-06 and LANG-05 (the
script crate's writer, in different functions), LAY-45 (`canvas/`).
DIAG-06a follows LANG-06 because both edit `apiforge-script/src/edit.rs`,
and LAY-45 because the gestures are tested against the sockets' final
place. DIAG-02 (#145, REQ) is not open together with LAY-45, DIAG-06a or
DIAG-06c: they share `canvas/`. `reference.md` is touched in different
sections; each ticket merges `main` before its last push.

**Owner minutes** still ahead on 2026-11-20: about 215 (LANG-13 30,
LANG-20 15, LANG-06 15, LANG-05 10, LAY-45 20, DIAG-06a 25, DIAG-06c 20,
LANG-21 25, DIAG-06b 15, REL-05 15, and the acceptance captures).

**Open beside the sprint:** #489, a Windows timing failure of
`mcp08b::take_back_waits_for_the_agents_run_and_its_cleanup` on #486's
first attempt; rerun once and noted there until its cause is read.

### Exit criteria of the first half

They are REL-05's release conditions:

- [ ] Every `.afs` of the repository passes `check`, prints unchanged, and
      its simulated run lists the same steps in the same order through the
      command line and the MCP server.
- [ ] The fix was applied to every scenario, fixture and example of the
      repository and to the owner's test projects; the order of steps is
      the same before and after; no comment was lost.
- [ ] Under `on_error continue` a chain goes on after an error and stops
      only at a missing value (RUN-01's eight criteria).
- [ ] 0.1.0 refuses a project saved by 0.2.0, and 0.2.0 does not run a
      project of language 1 before the fix; each says why.
- [ ] No gesture on the board writes a file that does not pass `check`.
- [ ] The reference describes language 2 only; ADR 0003, ADR 0005 and
      ADR 0010 are amended by the tickets that change them.

## Second half — the language grows

| Order | Ticket | Note |
| --- | --- | --- |
| 1 | LANG-16 #413 part B: the corpus of 15–20 scenarios, then blocks, loops, block results, scopes; mock-up of a block on the board | Analysis; decides the shape of everything below |
| 2 | LANG-01 #127 with LANG-09 #353 a missing field is a failure, not an error; a value from whichever branch ran | One analysis: both ask for the same thing, a value that may be absent and a way to say what stands in for it |
| 3 | LANG-08 #352 with LANG-07 #212 repeat for every element; repeat until a state holds; **LANG-17 #470** a table of cases, or a request based on another | One block shape, two conditions for going round again; RUN-02 (#130, a test over a table) is read with them. A table of cases may be that loop over a list of rows or a form of its own: the analysis says which, and LANG-16's corpus includes `labels` so that it can |
| 4 | LANG-02 #129 with LANG-04 #132 every, some, map, filter in checks; a partial match in a collection; **LANG-19 #472** a check without a name, one check for several fields | A partial match is "some element where…"; one check for several fields is a partial match of a whole body; one analysis |
| 5 | **LANG-18 #471** headers and authorization shared by a scenario | Its own analysis: where they are written, how a request leaves one out, precedence with a component (HTTP-02 #133). REQ-03 keeps headers per request until then |
| 6 | LANG-03 #131 text functions, comparison without case | S, additive, the pattern of LANG-10 |
| — | DIAG-05 #374 | As scheduled in LAY section 12.3 |

After LANG, as the owner scheduled: EDIT-01 (#358), CLI-01 (#359), EDIT-02
(#360) with EDIT-03 (#369). CLI-01 and LANG-13 both add a command; LANG-13
adds only `apiforge fix`.

Exit criteria of the second half are written by LANG-16.

## The older tickets, placed

| Ticket | Was | Now |
| --- | --- | --- |
| LANG-05 #142 | "`if` takes one condition" | A defect of the writer, not a gap: `if a == 1 && b != 2 {` is read by 0.1.0 and printed back as `(…) == true`. Retitled, S, first half |
| LANG-01 #127 · LANG-09 #353 | two tickets | one analysis after LANG-16 |
| LANG-08 #352 · LANG-07 #212 | two tickets | one analysis after LANG-16 |
| LANG-02 #129 · LANG-04 #132 | two tickets | one analysis; not the same thing as LANG-08 (checks, not steps) |
| LANG-06 #143 · KTLO-37 #211 | two tickets | one pull request, first half |
| LANG-11 #357 | closed | its reference text is replaced by LANG-14 |
| LANG-12 #385 | one successor by `@after` | closed 2026-11-17 as replaced by LANG-13 |
| RUN-01 #126 | backlog, "syntax to be found" | decided without syntax; first in the sprint |

## Decisions and records

| Decision | Where |
| --- | --- |
| The steps are one chain (eight decisions, four consequences) | [FLOW-00](../tickets/FLOW-00.md), #388 |
| The LANG quiz: the edge after an error, two parts of LANG-00, one release, the fix touches only `@after`, one formatter decision, one round on line order | #388, 2026-11-16 |
| Breaking changes allowed in 0.x; no several versions, no migration framework | the owner, 2026-11-16; [LANG-00](../tickets/LANG-00.md) |
| A board edit keeps what the author wrote | #396, #211 |
| A chain is written by the order of its lines (replaces FLOW-00's decisions 2 and 8 and consequence 1); `formatVersion: 2` with `languageVersion`; an older project runs only after the fix | [LANG-00](../tickets/LANG-00.md#owner-decisions-2026-11-16) |
| 2026-11-20, LANG-13: `shop-30` with every wire gets a new baseline, which LAY-45 brings back to 76 and 116; the header's summary is `N steps`. Planning rulings of the day: a component needs no `start`; a check that reads no step runs; a refused file is converted by hand so that what runs stays the same; the build reads language 2 from LANG-13 on | #386, [LANG-13](../tickets/LANG-13.md) on its branch |
| 2026-11-20, LANG-05: the short form of a check is canonical for a comparison or a logical operation; a bare value keeps `== true` | #142 |
| 2026-11-20, LANG-21: the window offers the update in 0.2.0; nine answers on its mock-up (the band, a check before writing, *Save all and update*, the list with *Show*, the count follows each save) | #482 |
| 2026-11-20, DIAG-06: sixteen answers on its mock-up, all the recommended option; split into DIAG-06a, DIAG-06c and DIAG-06b; LAY-45 moves into this sprint before them; bands (LAY-13) after 0.2.0; the keys of *Move earlier* and *Move later* are Alt+↑ and Alt+↓ | #391, #490, #491, #468 |
| 2026-11-20: the conversion of a project is whole or not at all (after an independent review of the ticket files) | #481, #411 |

ADRs the first half amends: [0003](../decisions/0003-start-reachability.md)
(a flow with a step has a start; a step outside the flow is an error),
[0005](../decisions/0005-scenario-language.md) (ordering, the braces rule,
what line order means, the three text operations),
[0010](../decisions/0010-folder-projects.md) (the manifest's language
version). What a blocked step blocks is [ADR 0015](../decisions/0015-run-after-an-error.md),
written by RUN-01.
