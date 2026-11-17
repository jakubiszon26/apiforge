# Handoff

One file, overwritten at each handoff. It says where the work stands when a
working day ends, so that the next session needs nothing but the
repository and the issues. State lives in the issues and pull requests;
this page points at them and orders the next day.

- Written: 2026-11-17, about 00:30 local (2026-11-16 23:30 UTC), by the
  planning model (Fable 5.2).
- `main`: `dd44c05`. Everything below was read from `git` and `gh` at that
  moment; hashes of pull requests are their heads.
- This file is on the branch `docs/handoff-2026-11-17`, not yet on `main`
  (no pull request: the limit of four is reached).

## 1. The LAY sprint

Plan: [sprints/lay.md](sprints/lay.md), sections 11 and 12. Backlog:
[backlog.md](backlog.md#sprint-lay-in-progress-since-2026-11-05).

- **Closed**: part A and the checkpoint work — the issues LAY-01 to
  LAY-11 (LAY-04 without a merge), LAY-16, LAY-20 to LAY-28, LAY-32
  (#398), LAY-33 (#404); LAY-10 merged last (#401). Beside
  LAY, merged on 2026-11-16: LANG-10 `uuid()` (#402), UX-23 (#406), MCP-23
  (#403), the CI change (#408).
- **In review** (section 2): RUN-03 #409, DIAG-07 #400, LANG-14 #415, and
  the draft LAY-29 #405.
- **Waiting**, open issues with `sprint:lay`: LAY-13 #297, LAY-14 #298,
  LAY-15 #299, LAY-17 #301, LAY-18 #302, LAY-19 #303, LAY-30 #383, LAY-31
  #384, LAY-34 #396, LAY-35 #410, and the acceptance LAY-12 #296.
- **Order of part B** (owner, 2026-11-16): LAY-10 → 14 → 18 → LAY-34 → 13 →
  19 → 17 → 15, then LAY-12. LAY-29, LAY-30 and LAY-31 come before the
  acceptance because they change what the owner judges.
- **To the end of the sprint**: the tickets above → LAY-12 (the demo and
  the timed task sheet, the owner's acceptance) → the retrospective
  ([process](process/retrospective.md)). None of the five exit criteria in
  the backlog is ticked yet.
- **The LANG sprint is paused** by the owner (2026-11-17) until LAY is
  closed, accepted and reviewed. No language design round and no LANG
  implementation before that, except what LAY needs: RUN-03 #389, DIAG-07
  #392, LANG-14 #390, LAY-34 #396 (LANG-10 is merged). Do not widen LAY
  without the owner.

## 2. Open pull requests

Four are open, so **the limit is reached** (rule 5 of AGENTS.md: four pull
requests, 60 review minutes; the three ready ones are 55 minutes). Address
review and CI on these; start no new pull request until one merges.

| Pull request | Head | Worktree | State at handoff |
| --- | --- | --- | --- |
| **#409** RUN-03 checks before the next step · `feat/run-03-checks-before-next-step` | `eb3943e` | `.claude/worktrees/run-03` | `main` merged in; no conflict. A semantic conflict with MCP-23 was fixed in `crates/apiforge-cli/tests/mcp.rs` (the test looked for `steps[0]` with outputs; it now finds the step by its handle `product`). `fmt`, `clippy` and `convert-v1` passed locally; **the full `cargo test` did not finish**. CI on `eb3943e` pending; the previous head `3786fbe` was green. No review thread |
| **#400** DIAG-07 a step added from *Next* · `fix/diag-07-step-after-chosen` | `da87d7f` | `.claude/worktrees/agent-a013d3a6bba89d966` | `main` merged in; no conflict. CI pending. One desktop test failed locally, cause not found: `folder::tests::acceptance_tests::issue349::issue349_criterion_1_sockets_of_a_card_are_a_gap_apart` (a flake or a defect of `main`; see section 6). Its agent reports CodeQL alert 46 fixed by `4e4dd86` |
| **#415** LANG-14 a step after a whole `if`; a chain stays in its braces · `feat/lang-14-after-whole-if` | `bd0a7f9` | `.claude/worktrees/agent-a893f5f75f9136a44` | `main` merged in; no conflict. `fmt`, `clippy`, `convert-v1` passed; **the full `cargo test` and the fuzzers `fuzz_syntax` and `fuzz_semantics` did not finish** (required for a language change). CI pending |
| **#405** LAY-29, draft · `feat/lay-29-no-wire-under-own-card` | `adecdf8` | `.claude/worktrees/agent-af5de1b8fec0a8df6` | **Conflicts with `main`** (22 commits behind); CI fails on Linux and Windows. Carries the implementation of option A, which the owner's later decision drops (section 5) |

All five worktrees were clean and pushed at handoff (nothing ahead of or
behind their remote branch), as was `/home/jakub/Projekty/apiforge-lang00`.

Also true at handoff:

- **No automatic review.** cubic reached its monthly limit ("AI review
  line limit reached"; it resumes on 2026-12-01). The rule "list unanswered
  cubic comments before a merge" finds nothing to list until then, which
  proves nothing. Until December a pull request gets a code review by an
  agent or by hand, written on the pull request, before it is merged.
- **One open CodeQL alert on `main`**, not from these pull requests:
  `rust/non-https-url`, `crates/apiforge-runner/src/http.rs`.
- **CI of `main`** was still running for `dd44c05`, `fde5d11` and
  `fffef9c`. Windows runs only on `main` now (#408), so watch these.
- Two processes seen during the day that were not ours — a
  `cargo test --no-fail-fast` (PID 2645241) and a zombie (PID 2580065) —
  no longer existed when this was written.

## 3. Merge order and what depends on what

1. **#409 first.** It changes the order a run evaluates, and tests of the
   other two read that order.
2. Then **refresh #400 and #415 with `main`** — merge `main`, never rebase —
   and run again what depends on the order: for #400 the core tests
   `diag07` criteria 1 and 2 and the desktop test
   `diag07_a_request_added_from_carts_next_runs_after_cart`; for #415
   `crates/apiforge-runner/tests/acceptance.rs`, which #409 changes too.
3. **#415 before LAY-34** (same printer), **LAY-34 before LAY-13** (the
   band reads which statements stand in a branch's braces).
4. **LAY-19 after #409.**
5. **LAY-29, then LAY-31**: one agent, the same `route.rs`. LAY-30 after
   LAY-31, LAY-35 after LAY-30.
6. After every merge, look at the Windows run of `main`.

Pull requests that were green alone have broken `main` together before:
refresh with `main` before each merge and check that `Closes #` names the
right issue.

## 4. Tomorrow, in order

Nothing below was started.

1. **Check the three pull requests.** CI of #409, #400, #415. Finish the
   local verification in each worktree: `cargo test --workspace
   --all-targets --locked`; for #415 the fuzzers `fuzz_syntax` and
   `fuzz_semantics`, 60 s each ([development guide](development.md));
   reproduce `issue349_criterion_1` on `main` to tell a flake from a
   defect. A review of each by an agent or by hand, since cubic is silent.
   Ask the owner about the `ci:windows` label on #409 (section 7).
2. **After the owner accepts and merges #409**, refresh #400 and #415 as
   in section 3.
3. **Narrow #405** to the half of LAY-29 that does not depend on the
   language (the decision is on #382) — after the question in section 7 is
   answered. Files: `apiforge-layout/src/route.rs`,
   `apiforge-desktop/src/canvas/routing.rs`, ADR 0006, the ticket file.
   It must first be brought up to `main`.
4. **Mock-ups, outside the limit** (branches `mock/…`, no pull request):
   LAY-31 — `shop-30`, checks expanded, before and after; LAY-30 — the
   switch on the board's bar and the switch on a step. Each is posted to
   its issue for the owner's quiz.
5. **LAY-14** #298 when a place is free: the cards track,
   `apiforge-desktop/src/canvas/`; collides with nothing open.
6. **LAY-34** #396 after #415 is merged. `apiforge-script` `print.rs`,
   `lower.rs`, `ast.rs`. Decided: a statement added from the board is
   written directly after the step it follows, inside the same braces;
   criterion 3 falls away. A `format` ticket: handed over by the
   coordinator, not taken unattended.
7. **LAY-19** #303 after #409: first the design-review view for the
   owner's approval, then the tests.
8. **LAY-29 → LAY-31**, then LAY-30, LAY-35; then LAY-18, LAY-13, LAY-17,
   LAY-15 in the owner's order.
9. **LAY-12**: the owner's acceptance, then the retrospective. Only then
   the LANG sprint.

## 5. Unfinished work, mock-ups and approvals owed

- **#405 holds work the later decision does not want.** On 2026-11-16 the
  owner chose option A on the mock-ups (an ordering line leaves its bottom
  socket and goes round the card), and it was implemented (`3951634`,
  `1aae304`, `adecdf8`, with a change of ADR 0006). Later the same evening
  the owner decided to deliver only the half that does not depend on the
  language. The planning model's question that led to the second decision
  described #405 as still waiting for A or B; it did not say that A was
  chosen and built. See section 7 before anything is removed.
- #405 also has an open defect of its own: `lay06_criterion_6_deterministic`
  fails in a serial run, and its committed `after-*.png` are older than its
  last change (noted on #382).
- **Local verification unfinished**: the full test run on #409 and #415,
  the fuzzers on #415.
- **Mock-ups owed**: LAY-31, LAY-30, LAY-19's view, REP-07 with UX-19.
  DIAG-06's mock-up is paused with the LANG sprint.
- **The owner's acceptance owed**: #409, #400, #415 (he merges, after the
  QA note in each); the quizzes on the mock-ups above; LAY-12.
- **Two documentation branches without a pull request**, waiting for a
  free place: `docs/lang-00-part-a` (`b350974`) and this one. Both touch
  `docs/backlog.md` and `docs/sprints/lay.md`, in different places.

## 6. Open problems and risks

- **Semantic conflicts in the merge train.** One was caught today (#409
  against MCP-23). Section 3 is the guard.
- **No cubic until December**: a merge without a written review is a merge
  without a review.
- **Full test runs did not finish locally** on two pull requests; CI is the
  only complete run so far, and it runs Linux only on a pull request.
- **#409 is not tested on Windows before the merge** unless it gets the
  `ci:windows` label; it changes the engine and `fixtures/runs`.
- **`issue349_criterion_1`** failed once, unexplained. The ticket for
  tests that fail now and then, #122, is closed; a repeat needs a new one.
- **The reference scenarios change at LANG-13.** Figures measured on them
  now (LAY-01's baseline, LAY-29's costs) are measured again then. What
  the language decisions change for LAY is in section 12.5 of the LAY plan
  on `docs/lang-00-part-a`.

## 7. Questions for the owner

1. **#405, LAY-29.** Option A is already implemented on the branch. Does
   the decision to narrow the ticket stand — the socket change is removed
   and only "no wire under its own card" is kept — or is A kept because it
   is built? Until LANG-13 the fan from *Start* stays on real boards.
2. **#409.** Add the `ci:windows` label before the merge?

## 8. Where the language decisions are

Everything is approved; nothing of it is implemented beyond LANG-10,
RUN-03 and LANG-14.

On the branch **`docs/lang-00-part-a`**, head **`b350974`** (pushed; worktree
`/home/jakub/Projekty/apiforge-lang00`), not yet on `main`:

- `docs/tickets/LANG-00.md` — the analysis and the record of the decisions.
- `docs/sprints/lang.md` — the sprint plan: order, files, what may run side
  by side, exit criteria, the older tickets placed.
- `docs/tickets/FLOW-00.md` — the amendment in its header.
- `docs/backlog.md` — the section *Next sprint: LANG (not started)*.
- `docs/sprints/lay.md` — section 12.5, what the decisions change for
  part B.

The decisions, each also on its issue:

| Decision | Issue |
| --- | --- |
| LANG-00 part A approved (three decisions on 2026-11-16, four confirmations on 2026-11-17) | #411 |
| **Language 2**: the steps of a block run in the order written; `@after` leaves the language; a step written in a block runs; checks and values stand anywhere; the step after an `if` runs after the whole `if`. Replaces FLOW-00's decisions 2 and 8 and its consequence 1 | #386 LANG-13, #388 |
| The document stays a graph of ordering edges — an architectural requirement, so that a later parallel block is not shut out. Running in parallel is not designed and not implemented now | #411, #386 |
| LANG-12 closed as replaced by LANG-13 | #385 |
| **RUN-01**: a chain edge means "after it finished"; under `on_error continue` only a missing value blocks; a value that existed when a component call failed can be read; a field missing from an answer stays an *error*; the cleanup goes on after a skipped step | #126 |
| **The language version**: `languageVersion` in the manifest, which becomes `formatVersion: 2`; a build reads one version; a newer project is refused with both versions named; an older one opens, is checked and edited, and runs only after the fix | #412 LANG-15 |
| **One release, 0.2.0**, with LANG-13, DIAG-06, RUN-01, RUN-03, LANG-14, LANG-15. Breaking changes are allowed in 0.x; no several language versions, no migration framework, no rollback; the fix keeps comments, writes a file whole or not at all and raises the version last | #414 REL-05 |
| **Edit, fix, format** are three operations; nothing formats as a side effect. One canonical attribute order for `print` and `fmt`; the parser does not require it and a board edit does not impose it | #411, #143, #211, #396 |
| LANG-05 is a defect of the writer (a compound `if` condition printed with `== true`), not a gap of the language | #142 |
| Part B of LANG-00 — a corpus of 15–20 scenarios a tester cannot write today, then blocks, loops, block results — after 0.2.0 | #413 LANG-16 |
| LAY-29 narrowed; LAY-34 writes a new statement directly after the step it follows | #382, #396 |
