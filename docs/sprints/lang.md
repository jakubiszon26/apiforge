# LANG: the language sprint — plan

- State: **proposed 2026-11-16**. The order below is the owner's (LANG
  quiz of 2026-11-16, recorded on
  [#388](https://github.com/jakubiszon26/apiforge/issues/388)), and the
  three decisions of [LANG-00](../tickets/LANG-00.md) are taken: a chain is
  written by the order of its lines, the manifest carries the language
  version under `formatVersion: 2`, a project in the older language runs
  only after the fix.
- **Not started.** Paused by the owner on 2026-11-17 until LAY closed; LAY
  closed on 2026-11-19 (#296) and the owner confirmed the same day that
  part A, to 0.2.0, runs next and the [REQ sprint](req.md) after it. The
  sprint starts after the LAY retrospective; until its planning no ticket
  of this plan carries a `ready:*` label.
- **Candidates raised on 2026-11-19, not decided** — the owner places them
  at the sprint's planning: LANG-17 (#470) a table of cases, or a request
  based on another, to be analysed with LANG-08; LANG-18 (#471) headers and
  authorization shared by a scenario; LANG-19 (#472) a check without an
  invented name and one check for several fields, with LANG-04. They come
  from the reference scenario `labels` (FIX-01, #454), whose sixteen
  `@after` lines are also a case for LANG-13's fix.
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

Paused until the sprint starts.

| Order | Ticket | Size | Needs | Files |
| --- | --- | --- | --- | --- |
| 1 | RUN-01 #126 the chain goes on after an error; only a missing value blocks | M · 20 · `default` | LANG-00 approved; #409 merged (same file) | `apiforge-runner/src/engine.rs`, `apiforge-core/src/run.rs`, `fixtures/runs`, `reference.md` (*on_error*) |
| 1 | LANG-15 #412 the language version | S–M · 20 · `format` | decided | `apiforge-storage/src/project/schema.rs`, `apiforge-project`, `apiforge-cli`, `apiforge-mcp` (`project_overview`, refusals), `reference.md` (first line) |
| 2 | LANG-13 #386 the steps of a block run in the order written; `@after` leaves the language; the fix and `apiforge fix` | M · 30 · `format` | LANG-14, LAY-34 and LAY-12 merged; LANG-15 (the fix raises the version). It changes lowering, printing and validation, not the document model | `apiforge-script` `lower.rs`, `print.rs`; `apiforge-core/src/validation.rs`; `apiforge-cli`; `reference.md`; every `.afs` under `fixtures/` and `docs/` |
| 2 | DIAG-06 #391 the board keeps the chain | from the mock-up · `visible UI` | mock-up approved; DIAG-07 (#392) merged | `apiforge-core/src/edit/nodes.rs`, `apiforge-desktop/src/canvas/` |
| 4 | LANG-06 #143 with KTLO-37 #211 Format keeps free comments; one attribute order | S · 15 · `format` | LANG-13 merged (same writer) | `apiforge-script` `format.rs`, `parser.rs`, `print.rs`; `reference.md` |
| 4 | LANG-05 #142 the window prints a compound `if` condition as written | S · 10 · `format` | LANG-13 merged | `apiforge-script` `print.rs` |
| 5 | REL-05 #414 release 0.2.0 | — | all above | release notes |

**Side by side.** Rows with the same order number touch different crates:
RUN-01 (runner) beside LANG-15 (storage, command line, MCP); LANG-13
(script, validation) beside DIAG-06 (edit, desktop). `reference.md` is
touched by five tickets in different sections; each merges `main` before
its last push. LANG-13 changes every fixture, so nothing else that reads
`fixtures/scenarios` is open when it lands — that is why it waits for
LAY-12.

**Owner minutes**, without DIAG-06 and the mock-up: about 95.

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
| 3 | LANG-08 #352 with LANG-07 #212 repeat for every element; repeat until a state holds | One block shape, two conditions for going round again; RUN-02 (#130, a test over a table) is read with them |
| 4 | LANG-02 #129 with LANG-04 #132 every, some, map, filter in checks; a partial match in a collection | A partial match is "some element where…"; one analysis |
| 5 | LANG-03 #131 text functions, comparison without case | S, additive, the pattern of LANG-10 |
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

ADRs the first half amends: [0003](../decisions/0003-start-reachability.md)
(a flow with a step has a start; a step outside the flow is an error),
[0005](../decisions/0005-scenario-language.md) (ordering, the braces rule,
what line order means, the three text operations),
[0010](../decisions/0010-folder-projects.md) (the manifest's language
version). No ADR states what a blocked step blocks; RUN-01 records its
rules in the one it finds closest, or in a new one.
