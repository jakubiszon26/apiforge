# MCP-02: A language reference the model reads from the server

- Size: M (mostly documentation) · Owner review: 30 min · Risk: none
- State: Backlog (issue #61) · MCP-00 decision 7 = A taken 2026-10-27
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md)

## Goal

A model that has never seen `.afs` writes a correct scenario after reading
one page: the reference it gets from the server is complete for what the
window accepts, matches the binary that serves it, and is proven by tests to
parse.

## What the owner will see

- New: `docs/language/reference.md`, the `.afs` reference for people and
  models, with a version line; the tool `language_reference` and the
  resources `apiforge://reference` and `apiforge://source/<path>` in Claude
  Code (`@apiforge:apiforge://reference` in a prompt attaches it).
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
   and its predicates (`==`, `~= … within`, `nonempty`, …), `if … else`,
   `cleanup`, `export`, component calls with `Handle@revision(…)`, comments
   and labels, and what cannot be expressed (layout). It ends with one
   complete scenario and one complete component, and a list of the
   diagnostics a model meets most and what to do about each.
2. The reference opens with `Reference version: 1 · grammar of ADR 0005 as
   of <date>`; `language_reference` and `project_overview` return that
   version and the crate version.
3. A test extracts every fenced `afs` block of the reference, parses it with
   no error, formats it unchanged (idempotent canonical form), and lowers the
   complete scenario and component against a fixture project that publishes
   the components they call — so the reference cannot drift from the
   grammar without failing the suite.
4. The reference is embedded with `include_str!`, so the text the tool
   returns is the text of the binary; the test compares them.
5. `resources/list` returns `apiforge://reference` and one
   `apiforge://source/<path>` per registered document; `resources/read`
   returns the reference or the saved text as `text/plain`.
6. The reference is under 50 000 characters, so Claude Code shows it inline
   instead of saving it to a file; a test asserts the length.
7. The verification commands of the development guide pass; ADR 0005 and the
   roadmap are amended as above.

## Not in scope

Completion, hover and go-to-definition for a model (`complete::describe`
exists; nothing asks for it yet). A Polish version. An MCP prompt (slash
command) that writes a scenario — once the reference exists a prompt is one
line of code and can join MCP-03 or a later ticket.

## Technical appendix

- Source of truth: ADR 0005 sections *Lexical structure* to *Comments and
  labels* (lines 58–236), the printer (`apiforge-script/src/print.rs`) for
  the canonical form, `complete.rs::predicate_words`/`method_words` for the
  complete lists, and the two canonical examples `fixtures/scripts/checkout.afs`
  and `lookup.afs`, which the reference may reuse verbatim.
- Test: `crates/apiforge-mcp/tests/reference.rs`; fenced blocks tagged `afs`
  are parsed with `apiforge_script::parse` and `format_tree`; the complete
  examples lower with `LowerContext` over a project built with
  `prepare_project` plus `publish`.
- Resources in `rmcp`: `ServerHandler::list_resources`/`read_resource`; the
  `resources` capability joins `tools` in the server info of MCP-01.
- Expected size: reference 250–350 lines, test 80, resources 60, ADR and
  roadmap amendments 10 — an M by lines, of which the owner reads the
  reference once.
