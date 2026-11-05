# Fixtures

| Path | Contents |
| --- | --- |
| `projects/*.json` | The demo projects: shop, tickets, operator blocks and the ordered workflow |
| `workspace.json` | A workspace containing all of them |
| `runs/*.json` | Simulation runs: every demo scenario and hand-built graphs (IF branches, graphs without Start, error policies, conditions, components, cleanup, diagnostics, redaction). Each file holds the run request and the normalised report, replayed by `crates/apiforge-runner/tests/run_corpus.rs` |
| `semantics/*.jsonl` | Expected results of operators, field paths and number formatting, one case per line, replayed by `crates/apiforge-core/tests/semantic_corpus.rs` |
| `scripts/*.afs` | Canonical scenario and component examples of the scenario language |
| `scenarios/lay/*.afs` | The reference scenarios of the LAY sprint, written by hand; see below |

## Reference scenarios

`scenarios/lay/` holds `order-flow` (14 steps), `shop-30` (30) and `shop-50`
(56), which the [LAY tickets](../docs/sprints/lay.md) measure and test against.
A step is a node of the scenario's main graph: the start, every request, value,
component, check and condition, a branch and both of its arms. The `cleanup`
block is a second graph, counted apart (two steps in each scenario).

They call components of the demo shop project and open there, as
`crates/apiforge-runner/tests/acceptance.rs` does: it holds what the later tickets
read from `shop-30`, so a change that breaks a ticket fails there. The
[quality command](../docs/development.md#verification) prints their board metrics.

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

## Semantic corpus

The semantic corpus was recorded with a JavaScript engine, whose number and value
semantics documents follow ([ADR 0001](../docs/decisions/0001-numeric-semantics.md)).
It is frozen: a mismatch means expression behaviour changed. Fix the code, or record a
deliberate divergence in an ADR together with the corpus change.
