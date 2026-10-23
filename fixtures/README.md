# Fixtures

| Path | Contents |
| --- | --- |
| `projects/*.json` | The demo projects: shop, tickets, operator blocks and the ordered workflow |
| `workspace.json` | A workspace containing all of them |
| `runs/*.json` | Simulation runs: every demo scenario and hand-built graphs (IF branches, graphs without Start, error policies, conditions, components, cleanup, diagnostics, redaction). Each file holds the run request and the normalised report, replayed by `crates/apiforge-runner/tests/run_corpus.rs` |
| `semantics/*.jsonl` | Expected results of operators, field paths and number formatting, one case per line, replayed by `crates/apiforge-core/tests/semantic_corpus.rs` |
| `scripts/*.afs` | Canonical scenario and component examples of the scenario language |

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
