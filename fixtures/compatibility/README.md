# Semantic corpus

This directory holds the expected results of the reference semantics, generated from
the TypeScript prototype by `scripts/export-compatibility-fixtures.ts`:

- `semantics/*.jsonl` — operators, field paths and number formatting, one case per
  line,
- `manifest.json` — SHA-256 checksums of every generated file.

The corpus is replayed by `crates/apiforge-core/tests/semantic_corpus.rs`. A mismatch
means the Rust port changed the result of an expression: fix the port, or record a
deliberate divergence in an ADR under `docs/decisions/` (see ADR 0001). Never edit the
corpus by hand.

Project and run fixtures are no longer generated here. Since document format version 2
they are native fixtures in `fixtures/` (see `fixtures/README.md`).

## Generating and checking

```bash
node --import tsx scripts/export-compatibility-fixtures.ts
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Exit codes: `0` when every file on disk is byte-identical with the generated set, `1`
when a file is missing or differs, `2` for unsupported command-line arguments.
