# Fixtures

| Path | Contents |
| --- | --- |
| `projects/*.json` | The demo projects: shop, tickets, operator blocks and the ordered workflow |
| `workspace.json` | A workspace containing all of them |
| `runs/*.json` | Simulation runs: every demo scenario and hand-built graphs (IF branches, graphs without Start, error policies, conditions, components, cleanup, diagnostics, redaction). Each file holds the run request and the normalised report, replayed by `crates/apiforge-runner/tests/run_corpus.rs` |
| `semantics/*.jsonl` | Expected results of operators, field paths and number formatting, one case per line, replayed by `crates/apiforge-core/tests/semantic_corpus.rs` |
| `scripts/*.afs` | Canonical scenario and component examples of the scenario language |
| `scenarios/lay/*.afs` | The reference scenarios of the LAY sprint, written by hand; see below |
| `demo/` | The folder project a first start writes (DEMO-01), written by hand; see below |

## Reference scenarios

`scenarios/lay/` holds `order-flow` (14 steps), `shop-30` (30), `shop-50`
(56), `fan-out` (26) and `labels` (50), which the
[LAY tickets](../docs/sprints/lay.md) measure and test against. `fan-out` is shaped like a board where a component call gives
two values that almost every later request reads (LAY-23); it has no cleanup.
`far-value` (23) is the same board with no variable, its cart and session
values read directly by far steps, and `far-value-saved` (26) the same with
those values saved in variables (LAY-25). `labels` is shaped like a
validation test: sixteen requests to one endpoint of a fictional parcel API,
each followed by one to four checks, and no request reads another (FIX-01). It
calls no component and has no cleanup; `crates/apiforge-runner/tests/fix01.rs`
lowers it in a folder project of its own, and the desktop's tests open it
through `labels_board` of `src/testing.rs`.
A step is a node of the scenario's main graph: the start, every request, value,
component, check and condition, a branch and both of its arms. The `cleanup`
block is a second graph, counted apart (two steps in each of the other three).

They open in the demo shop project, whose components `shop-30` and `shop-50` call,
as `crates/apiforge-runner/tests/acceptance.rs` does: it holds what the later
tickets read from `shop-30`, so a change that breaks a ticket fails there. The
[quality command](../docs/development.md#verification) prints their board metrics.

`scenarios/lay/rooms/` is a folder project, not a single file: a fictional
room-booking API with one component (`sign_in`, and its published revision
under `.apiforge/revisions/`) and three scenarios, `double-booking` (11
steps), `double-booking-notice` and `booking-day` (29 steps). It is the
owner's acceptance material for the close of the LAY sprint (#296): two
bookings that read `rooms.body.items[0].id` and `rooms.body.items[1].id`.
A test copies the folder to a temporary directory and opens the copy as a
project; nothing walks it with the single-file references above.

## The first-start demo

`demo/` is *Demo shop* as a first start in an empty data directory writes it
([DEMO-01](../docs/tickets/DEMO-01.md)): the scenarios *Buy coffee on
promotion* (19 steps in a simulation, the `else` branch skipped) and *The cart
refuses bad orders* (three refused orders under `on_error continue`), the
component `product` and its published revision 1 under `.apiforge/revisions/`.
The desktop embeds these files as they are (`apiforge_core::demos::FIRST_START`),
so editing one changes what a new install opens; a file added here is added to
that list too. They are held to what the other working sources are held to:
`check` without a diagnostic, `fmt --check`, the recorded step order of
LANG-13 and the same order through the command line and MCP (REL-05), and
`crates/apiforge-cli/tests/demo01.rs` runs both scenarios live against the demo
API. No layout is kept here: the window lays each board out as *Tidy up* does
on its first open and saves it. `demos::shop_project()` is a different demo
shop, the one the tests build on.

## Generated fixtures

Project and run fixtures are generated from frozen version-one sources in
`tools/convert-v1/sources/` by the development converter
([ADR 0004](../docs/decisions/0004-document-format-v2.md)):

```bash
cargo run -p apiforge-convert-v1 --locked -- --fixtures          # regenerate
cargo run -p apiforge-convert-v1 --locked -- --fixtures --check  # verify, also a unit test
```

Never edit generated fixtures by hand. A change of the format or of run semantics
changes the converter; each resulting change of an expected report is explained in the
commit that makes it, and deliberate divergences from the recorded behaviour are listed
in `run_corpus.rs`.

The step order of a report in `runs/` is not recorded: the converter puts the steps in
the order `graph::sequence` gives, which is the code under test. `run_corpus.rs` therefore
proves statuses and values, not order; the order is proved by the RUN-03 and RUN-04 tests
(`crates/apiforge-runner/tests/run03.rs`, `run04.rs` and
`run03_criterion_5_sequence_agrees_with_the_engine` in
`crates/apiforge-core/tests/acceptance.rs`).

## Semantic corpus

The semantic corpus was recorded with a JavaScript engine, whose number and value
semantics documents follow ([ADR 0001](../docs/decisions/0001-numeric-semantics.md)).
It is frozen: a mismatch means expression behaviour changed. Fix the code, or record a
deliberate divergence in an ADR together with the corpus change.
