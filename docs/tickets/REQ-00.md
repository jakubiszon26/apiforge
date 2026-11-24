# REQ-00: One place for a request — the Steps view and the request beside its response: captures and decisions

- Size: S · Owner review: 30 min plus the quiz · Risk: `visible UI`, `default`
- State: **Decided.** The owner chose A on every board and took P4 on
  2026-11-24; the rules without a picture were ruled the same day by an
  independent agent on the owner's delegation of 2026-11-23
  ([#456](https://github.com/jakubiszon26/apiforge/issues/456)). The
  mock-up is approved as a whole, with its known defects turned into
  criteria.
- Analysis model: Fable 5.2 · Implementation model: — (no implementation)

## Goal

How a scenario is read as a list of steps and how one request is edited
with its response beside it, decided on captures of the real renderer.

## Mock-up

Throwaway branch `mock/req-00`, commit `93e2f89b`; the captures are under
`docs/qa/2026-11-24/REQ-00/` of that commit. Boards:
[1 SENDS](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q1-sends-960.png) ·
[2 Inspector](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q2-inspector-960.png) ·
[3 Send alone](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q3-send-alone-1600.png) ·
[4 fold](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q4-fold-960.png) ·
[5 kinds](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q5-kinds-1600.png) ·
[6 menu](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q6-menu-1600.png) ·
[7 add](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q7-add-960.png) ·
[8 Cleanup](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/q8-cleanup-1600.png).
Whole windows: [960 before](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-before-run-960.png) ·
[960 after](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-after-run-960.png) ·
[1600 after](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-steps-after-run-1600.png) ·
[rail](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-q4-a-rail-960.png) ·
[Body](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-request-body-1600.png) ·
[Headers](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-request-headers-1600.png) ·
[Auth](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-request-auth-1600.png) ·
[rooms](https://raw.githubusercontent.com/jakubiszon26/apiforge/93e2f89b755444cf1459834ab02cc978b0afa3f4/docs/qa/2026-11-24/REQ-00/window-rooms-after-run-960.png).

Defects of the mock, each a criterion of the ticket named, never copied:
`not_booked` listed under `first` (REQ-01a 3); the caption *SENDS ·
differs from the common request* on two lines (REQ-01a 4); texts cut at
a fixed count of characters with room left or none (REQ-01a 9); *Sample*
and *Settings* under *More* at 1600 (REQ-02a 5).

## Decisions

Decisions 5–9 and 14 of 2026-11-19 stand.

| | Decided | By |
| --- | --- | --- |
| 1 | SENDS lists what differs from the common request of the group | owner |
| 2 | Nothing leaves the Inspector; *Steps* has the request pane, *Canvas* is as today | owner |
| 3.3 | A single send is live only as a run would be: the run-mode switch and every gate of a run | owner |
| 3.4 | A single send's response is kept beside the run's, marked *SENT ALONE*; the verdict does not change | owner |
| 4 | At 960 × 540 the list folds to a rail | owner |
| 5 | Every step is a row | owner |
| 7 | `+` between cards and *Add step* at the end of Main and of Cleanup | owner |
| 8 | Cleanup is a section after Main; *Move to Cleanup* in the card's menu | owner |
| P4 | In *Steps* a run does not open Run results; the bar with the verdict stays | owner |
| 6 | A step moves by *Move earlier* / *Move later* and Alt+↑ / Alt+↓, no dragging; the moved card stays selected and in view | agent |
| 1 rules | The rules of SENDS: [REQ-01a](REQ-01a.md#the-rules-of-sends), their one owner | agent |
| 3.1 | A value the request reads is missing: refuse, name the step and the field, offer *Run to here*. Missing also: the last run was in the other mode; in a live send, a value the run holds only as `[REDACTED]` | agent |
| 3.2 | A document newer than the run is sent; the pane names the run the values come from. A document with errors is refused, never "the last valid version" | agent |
| 3.5 | The request's checks are evaluated on a single send and shown in the pane only | agent |
| 5.2 | A check is listed where it is written, in its branch; when that is not under the step it reads, its text keeps that step's name | agent |
| 8 rule | *Move to Cleanup* is refused, naming the reader, when a step or check that stays in Main reads the step; offered for a request and a component call; the step lands last in the section with its checks | agent |

The reasons, from the code and the tester's use, are in the ruling on
#456.

## What the code says about REQ-04 and REQ-05

The analysis claimed no engine work. Read against `main` at `1923fcd3`,
that does not hold:

- `target_node_id` selects `ancestors_and_self` (`apiforge-core/src/graph.rs:840`).
  Checks and values are data statements off the chain (`NodeKind::is_data`),
  so the checks of the steps on the way, and the target's own, do not
  run: `fixtures/runs/target.json` runs four nodes and not `verify`.
  "*Weight 0 kg* runs 15 steps" needs the checks.
- `single_node_id` runs one node and then **the whole Cleanup**
  (`engine.rs:412-430`; `fixtures/runs/single-workflow-add.json` sends the
  DELETE). A single send must run nothing else, and its checks.
- The folder window has one report per document (`folder/runs.rs`,
  `publish`) and opens the panel on every start; `consent.rs` is the
  agent's consent (MCP-09), not the person's. The person's live run is
  the run-mode switch (`chrome.rs`) and the gates of `request_run` and
  `start_run_with`: not observing, no recovery pending, one run at a
  time, a journal, `prepare_run`, `validate_run`.

Hence [REQ-04a](REQ-04a.md), the engine's two partial runs, before
REQ-04b and REQ-05. The frozen corpus and the workspace window are not
changed.

## Where the code lives

A new module `crates/apiforge-desktop/src/steps/`, beside `canvas/` and
`inspector/`; the folder window calls it from `folder/steps_view.rs`.
`canvas/mod.rs` and `app.rs` gain no line for *Steps*;
`folder/diagram.rs` gains the switch and the call only. Each file has
one ticket that creates it:

| File | Created by | Holds |
| --- | --- | --- |
| `steps/model.rs`, `steps/sends.rs`, `steps/list.rs`, `folder/steps_view.rs` | REQ-01a | rows from the graph, no egui in the first two; the cards |
| `steps/list_edit.rs` | REQ-01b | `+`, the *Add step* rows, the card's menu |
| `steps/panes.rs`, `steps/request.rs`, `steps/response.rs` | REQ-02a | the three columns and the rail, the two panes |
| `steps/body.rs` | REQ-02b | the body editor |
| `steps/headers.rs`, `steps/auth.rs` | REQ-03 | the two tabs |
| `apiforge-runner/src/partial.rs` | REQ-04a | the two selections |
| `steps/single.rs` | REQ-05 | what a single send lacks, and its result |

## Tickets and order

| Ticket | Issue | Size · review | After | Label |
| --- | --- | --- | --- | --- |
| [REQ-01a](REQ-01a.md) Steps reads | #457 | M · 30 | REP-07a (#588) | `ready:opus` |
| [REQ-04a](REQ-04a.md) the engine's partial runs | #599 | M · 10 | — | `ready:opus` |
| [REQ-01b](REQ-01b.md) Steps adds and moves | #600 | M · 20 | REQ-01a, DIAG-22 (#598), WS-05b (#596) | `ready:opus` |
| [REQ-02a](REQ-02a.md) the request and its response | #458 | M · 30 | REQ-01a, REP-07b (#381), REQ-15 (#576) | `ready:opus` |
| [REQ-14](REQ-14.md) Move to Cleanup | #549 | M · 15 | REQ-01b, DIAG-22 | `ready:opus` |
| [REQ-02b](REQ-02b.md) the body editor | #459 | M · 20 | REQ-02a | `ready:opus` |
| [REQ-03](REQ-03.md) headers and Auth | #460 | M · 15 | REQ-02a | `ready:opus` |
| [REQ-04b](REQ-04b.md) Run to here | #461 | M · 20 | REQ-02a, REQ-04a | `ready:opus` |
| [REQ-05](REQ-05.md) Send alone | #462 | M · 25 | REQ-04b | `ready:opus` |

In parallel, on disjoint files: REQ-01a with REQ-04a (first); then
REQ-01b with REQ-02a; then REQ-02b, REQ-03 and REQ-04b (`steps/body.rs`,
`steps/headers.rs` and `auth.rs`, `folder/runs.rs`); REQ-14 any time
after REQ-01b; REQ-05 last. Each adds its tab or button to a slot the
earlier ticket leaves in `steps/request.rs` or `steps/list_edit.rs`; the
second to merge refreshes with `main`. The limit of four pull requests
and 60 review minutes still holds.

Shared with tickets in flight: WS-05a (#574) owns the header's title and
its edit, REQ-01a the switch on the right of the same header
(`folder/diagram.rs`); REP-07a and REP-07b own how a check's row is
marked, which *Steps* calls and never redraws; REQ-15 owns *Add a sample
response*, which REQ-02a points at the *Sample* tab; DIAG-22 owns "a
step added from a step lands right after it", which REQ-01b calls.

## Proposals beyond the request

P1 and P4 are decided. P2 (a group read as a table), P3 (the common
request shown above its group) and P5 (Ctrl+Enter runs to here) wait for
the owner; none is in a ticket. New, from the ruling: a single send of a
request that reads a secret of the last run (a token from a login)
cannot be live, because a kept run holds `[REDACTED]`; candidate
[#601](https://github.com/jakubiszon26/apiforge/issues/601).

## Not in scope

Implementation; REQ-07 to REQ-13; the redesign of *Output data*.
