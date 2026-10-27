# MCP-02: A language reference for models, tested — QA report

- Branch and commit: `docs/mcp-02-language-reference`, `65913d4` (acceptance
  tests `8d5795e`, reference `a3d8b19`, ADR and roadmap `65913d4`)
- Built with: `cargo test --workspace --all-targets --locked`
- Compared against mock-up: not applicable, no interface change
- Analysis model: Fable · Implementation model: Opus 5.5 · Report written by
  the planning session from the implementing agent's evidence (its tool
  refused to create this file)

## Linux

### Scenario 1: the reference covers the language (criterion 1)
1. Open `docs/language/reference.md`.
2. Run `cargo test -p apiforge-project --test reference --locked`.
Expected: one section per topic of criterion 1, each with an `afs` example;
one complete scenario and one complete component at the end; a list of the
diagnostics a model meets most, naming `stale_source`,
`comments_would_be_lost`, `secret_in_text`, and that `check_text` checks a
proposal while `run` executes the saved source.
Observed: as expected; the test asserts the 16 `##` sections in order, an
example per topic section, and the diagnostics section's names. The
examples use every predicate of `predicate_words`, every method of
`method_words`, every operator and every statement and request form, read
from the parsed trees, not the prose.

### Scenario 2: the version line (criterion 2)
Expected: the file opens with `Reference version: 1 · grammar of ADR 0005 as
of 2026-10-27`.
Observed: as expected (asserted by the test).

### Scenario 3: every example parses, formats unchanged and lowers (criterion 3)
1. At `8d5795e` (tests against a stub reference): `1 passed; 6 failed` — only
   the length limit passes; the version line, sections, examples, server
   codes and complete examples are missing. The tests fail for the right
   reason.
2. At `a3d8b19` and later: `7 passed; 0 failed`.
3. Drift check by hand: extra spaces around `==` in the complete component
   and the call changed to `find_product@2`. The canonical-form test failed
   naming the block's line; the lowering test failed with `Component
   revision does not exist`. The file was restored.
Expected: every fenced `afs` block (17) parses with no diagnostic, warnings
included, and `format_tree` returns it unchanged; the complete component is
created in a fixture built with `prepare_project`, has no diagnostics, is
published as revision 1; the complete component and scenario lower with
`LowerContext::folder` with no diagnostics and every `find_product@1` call
resolves to the published revision.
Observed: as expected.

### Scenario 4: size (criterion 4)
Expected: under 50 000 characters.
Observed: 27 790 characters, 695 lines (the ticket estimated 250–350 lines;
every example is a complete file). Asserted by the test.

### Scenario 5: ADR 0005 and the roadmap (criterion 5)
Expected: ADR 0005's consequence says the reference is written and
maintained by hand and a grammar change updates the ADR, the reference and
the round-trip corpus together; the roadmap's M7 row no longer lists the
language reference.
Observed: as expected (`65913d4`). The M7 acceptance bullet "the language
reference is complete enough that a model can write a valid scenario from
it alone" stays: it is still the acceptance of that milestone, now met by
this document (planning decision).

### Verification commands
- `cargo fmt --all -- --check`: exit 0
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit 0
- `cargo test --workspace --all-targets --locked`: exit 0, 1117 passed,
  0 failed, 29 ignored
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit 0

## Windows

Not run on Windows: documentation and one test file; the test is
platform-independent and runs in CI on both runners.

## Not verified

- No model has been asked to write a scenario from the reference alone;
  MCP-11 is the end-to-end check.
- The server codes are described as MCP-00 designs them; the server does
  not exist yet (MCP-01, MCP-04, MCP-05).
- The topic-section examples only parse and format; they are not lowered,
  and some name a contract, an environment value or components that no
  fixture provides. Only the two complete examples lower.
- The complete scenario was not run.
- The Markdown rendering was not reviewed in a viewer.

## Regression smoke list

Not run: documentation and one test file change; no crate source changes.
