# Compatibility fixtures (v1)

This directory holds deterministic reference files generated from the TypeScript
prototype by `scripts/export-compatibility-fixtures.ts`:

- `projects/shop.json`, `projects/tickets.json`, `projects/blocks.json`,
  `projects/workflow.json` — demo projects in format v1,
- `workspace-v1.json` — a workspace export containing all of them,
- `semantics/*.jsonl` — expected results of the reference semantics (operators, field
  paths and number formatting), one case per line,
- `manifest.json` — SHA-256 checksums of every generated file.

## Purpose

The project fixtures guard backward compatibility of the saved project format (`Project`
v1) and of the workspace export. Random identifiers (`project-*`, `scenario-*`, `node-*`,
`order-*`) are mapped deterministically, in tree traversal order, to stable values
(`prefix-00000001`, …), and values of variables listed in `environment.secretNames` are
removed.

The semantic corpus is replayed by `crates/apiforge-core/tests/semantic_corpus.rs`. A
mismatch means the Rust port changed observable behaviour of existing projects: fix the
port, or record a deliberate divergence in an ADR under `docs/decisions/`. Never edit the
corpus by hand.

## Generating

```bash
node --import tsx scripts/export-compatibility-fixtures.ts
```

## Checking (e.g. in CI)

```bash
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Exit codes: `0` when every file on disk is byte-identical with the generated set, `1` when
a file is missing or differs or `validateProject` reports errors, `2` for unsupported
command-line arguments.
