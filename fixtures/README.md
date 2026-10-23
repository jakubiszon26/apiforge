# Fixtures

- `projects/*.json` — the four demo projects (shop, tickets, blocks, workflow),
- `workspace.json` — a workspace containing all of them,
- `runs/*.json` — simulation runs: every demo scenario plus hand-built graphs (IF
  branches, graphs without Start, error policies, conditions, components, cleanup,
  diagnostics, redaction). Each file holds the run request and the normalised report,
  replayed by `crates/apiforge-runner/tests/run_corpus.rs`,
- `compatibility/` — the semantic expression corpus generated from the TypeScript
  reference (see its README).

The project and run fixtures are generated from the version-one files the TypeScript
prototype exported, frozen in `tools/convert-v1/sources/`, by the development
converter ([ADR 0004](../docs/decisions/0004-document-format-v2.md)):

```bash
cargo run -p apiforge-convert-v1 -- --fixtures          # regenerate
cargo run -p apiforge-convert-v1 -- --fixtures --check  # verify, also a unit test
```

Never edit generated fixtures by hand. A change of the format or of run semantics
changes the converter; each resulting change of an expected report must be explained
in the commit that makes it, and deliberate divergences from the recorded reference
behaviour are listed in `run_corpus.rs`.
