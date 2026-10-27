# MCP-02: A language reference for models, tested

- Size: M (mostly documentation) · Owner review: 30 min · Risk: none
- State: Backlog (issue #61) · MCP-00 decision 7 = A taken 2026-10-27 ·
  revised 2026-10-27: the document and its tests only; the tool and the
  resources moved to MCP-03, which needs the server of MCP-01
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

A model that has never seen `.afs` writes a correct scenario after reading
one page: the reference is complete for what the window accepts, matches the
grammar of the binary that will serve it, and is proven by tests to parse,
format unchanged and lower.

## What the owner will see

- New: `docs/language/reference.md`, the `.afs` reference for people and
  models, with a version line.
- Changed: ADR 0005's consequence "the reference for users and MCP is
  generated from this ADR in M7" is amended: the reference is written and
  maintained by hand, and a grammar change updates the ADR, the reference and
  the corpus together. The roadmap's M7 row loses "language reference".
- Removed, migrated, defaults: nothing.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `docs/language/reference.md` covers, with one example each: file shape
   (`scenario`/`component` header, `description`, `on_error`), parameters and
   inputs (`param`, `input`, defaults, `@optional`, `@label`), `start` and
   `@after`, requests (method, path template `{{…}}`, `headers`, `query`,
   `body`, `timeout`, `sample`), expressions and operators, `var`, `assert`
   and its predicates, `if … else`, `cleanup`, `export`, component calls with
   `Handle@revision(…)`, comments and labels, and what cannot be expressed
   (layout). It ends with one complete scenario and one complete component,
   and a list of the diagnostics a model meets most and what to do about
   each — including `stale_source`, `comments_would_be_lost` and
   `secret_in_text` from the server, and the note that `check_text` checks a
   proposal while `run` executes the saved source.
2. The reference opens with `Reference version: 1 · grammar of ADR 0005 as
   of <date>`.
3. A test in `apiforge-project` extracts every fenced `afs` block of the
   reference, parses it with no error, formats it unchanged (idempotent
   canonical form), and lowers the complete scenario and component against a
   fixture project that publishes the components they call — so the
   reference cannot drift from the grammar without failing the suite.
4. The reference is under 50 000 characters, so Claude Code shows it inline
   instead of saving it to a file; the test asserts the length.
5. The verification commands of the development guide pass; ADR 0005 and the
   roadmap are amended as above.

## Not in scope

Serving the reference (`language_reference`, `apiforge://reference`) and the
source resources: MCP-03, after MCP-01. Completion, hover and go-to-definition
for a model. A Polish version. An MCP prompt that writes a scenario.

## Technical appendix

- Source of truth: ADR 0005 sections *Lexical structure* to *Comments and
  labels* (lines 58–236), the printer (`apiforge-script/src/print.rs`) for
  the canonical form, `complete.rs::predicate_words`/`method_words` for the
  complete lists, and `fixtures/scripts/checkout.afs` and `lookup.afs`, which
  the reference may reuse verbatim.
- Test: `crates/apiforge-project/tests/reference.rs`, reading the file with
  `include_str!("../../../docs/language/reference.md")`; fenced blocks tagged
  `afs` are parsed with `apiforge_script::parse` and `format_tree`; the
  complete examples lower with `LowerContext` over a project built with
  `prepare_project` plus `publish`.
- Expected size: reference 250–350 lines, test 80, ADR and roadmap
  amendments 10.
