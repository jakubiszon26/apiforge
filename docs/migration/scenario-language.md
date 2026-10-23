# M4: scenario language implementation

Updated 23 October 2026. Work is local on `feat/script-language`; M4 is in
progress and has not been merged into `main`.

## Implemented syntax foundation

`apiforge-script` provides a lossless token stream and a recovering parser for the
[ADR 0005 grammar](../decisions/0005-scenario-language.md). The original source,
whitespace, comments and invalid tokens remain available in `SyntaxTree`.
Declarations, statements, expressions, attributes and checks have byte ranges into
the UTF-8 source. Diagnostics have severity, ranges and insertion fixes for obvious
missing punctuation. They describe errors without echoing source literals.

The parser accepts scenarios and components, cleanup, typed inputs/exports,
requests, checks, IF/ELSE, object/array expressions, operator/component calls,
reference paths, JSON strings, dedented multiline strings and nested template
interpolation. Recovery keeps later statements after a malformed statement.
Directly adjacent leading comments and trailing line comments become statement
notes; unattached comments warn before the formatter drops them.

`format` and `format_tree` print canonical syntax: two-space indentation,
consistent spacing, notes above statements, explicit boolean checks and quoted
strings with JSON escapes. Explicit expression groups are retained. A successful
format produces text that can be parsed and formatted again without changing it.

Limits are 1 MiB of UTF-8 source, 64 levels of nesting/expression chains and 1000
parser diagnostics. File reads stop after the source limit plus one byte. Formatting
also refuses an output that would exceed the input size limit, preserving the
original rather than saving text that the next invocation cannot read. Comment
attachment uses binary search over source-ordered comments and stops at an
already attached comment; a 10,000-statement regression covers association at scale.

`apiforge-cli` exposes:

```bash
cargo run -p apiforge-cli --locked -- fmt fixtures/scripts/checkout.afs --check
cargo run -p apiforge-cli --locked -- fmt path/to/scenario.afs
cargo run -p apiforge-cli --locked -- check --workspace fixtures/workspace.json
cargo run -p apiforge-cli --locked -- print --workspace workspace.json --scenario <name-or-id>
cargo run -p apiforge-cli --locked -- apply --workspace workspace.json --scenario <name-or-id> file.afs
```

`--check` can precede or follow the filename. It never writes. Normal formatting
uses the storage crate's synced temporary-file/atomic-rename implementation and
avoids a write when the file is already formatted. Invalid syntax, invalid UTF-8
and oversized input/output leave the source untouched. Warnings and errors go to
stderr without source excerpts. Exit codes are 0 for success, 1 for diagnostics,
I/O or a formatting mismatch, and 2 for invalid arguments.

The checked-in [scenario](../../fixtures/scripts/checkout.afs) and
[component](../../fixtures/scripts/lookup.afs) examples are canonical syntax
fixtures. Both now lower to document models; CLI execution remains M6 work.

## Validation

The new tests exercise all syntax forms, precedence and associativity, nested
templates, exact interpolation spans, Unicode, dedenting, comment ownership,
recovery, punctuation fixes, duplicates, malformed numbers, durations and paths,
source/depth/diagnostic budgets, CLI exit codes, no-write behaviour and literal
redaction. Properties cover 10,000 generated expression trees (structure preserved
after formatting, ignoring ranges and redundant groups), 5000 mutated UTF-8 inputs
and formatting idempotence. These are syntax properties, not graph round trips.

The dependency-free `fuzz_syntax` example mutates the syntax fixtures and seeded
template/path/check examples. It checks panic freedom, token coverage, UTF-8 range
boundaries, fixes and formatting idempotence:

```bash
cargo run -p apiforge-script --example fuzz_syntax --release --locked -- 60 1592639710
```

This is a sustained mutational smoke fuzzer, not coverage-guided libFuzzer.
The final Linux release run completed 11,327,494 mutations in 60 seconds with
seed `1592639710`, including 15,692 syntactically valid cases, without a panic or
range/idempotence failure.

The Linux workspace checkpoint passes 302 tests (21 new), with one existing
ignored process-lock helper launched by its parent test. Formatting, Clippy for
all targets with warnings as errors, both compatibility-fixture checks and the
workspace debug build pass. Both example files pass the CLI's `--check` command.
The CRLF string regression was observed failing before the trimming correction
and passes with it. This checkpoint changes no GUI, so it adds no native GUI
acceptance claims.

## Semantic and CLI follow-up

`lower` compiles statements into version-two scenarios/components with temporary
handle identities. It resolves scopes, output ports, component handles/revisions,
operator slots, parameters and contracts, derives data dependencies from bindings
and creates explicit/implicit IF ordering edges. Main flow cannot reference
cleanup. Both graphs share the handle namespace. Errors omit the offending block
and prevent reconciliation or CLI apply. The shared core validator checks graph
cycles, limits, local request arguments and document structure.

`print_scenario` and `print_component` return checked canonical source or static
diagnostics. They preserve exact optional configuration, missing binding slots,
notes, input flags/actions, cleanup, pinned component revisions and extra inputs.
The grammar additions are documented in ADR 0005. Print verifies its own semantic
round trip. Existing unset environment names survive printing; strict workspace
checking still reports them.

`reconcile` keeps IDs and placement for unchanged handle/kind pairs. One equal
rename retains identity only when all references and ordering uses are updated.
Changed kinds and ambiguous insertions get fresh IDs. It returns the IDs needing
layout; `apply` currently places them beyond the existing board's right edge.
M2 will supply the shared geometry algorithm. Component reconciliation returns a
candidate definition; persisting a new revision remains the component editor's
existing responsibility.

`check`, `print` and scenario `apply` target the workspace's active project.
Scenario IDs take precedence; display names must be unique. Apply retains the
standalone-project or workspace envelope and other projects/definitions/scenarios.
It uses S1's exclusive directory lock, fingerprint guard and recovery gate, plus
atomic writes with a validated backup retaining its original envelope. Semantic
no-ops leave the file and backup untouched. CLI commands never restore a backup
implicitly, and `check`/`print` never write files.

The request inspector regression is fixed: **Arguments** has an Add control and
editable custom input ports. `{{input.name}}` feeds URL/query/header templates;
the **Request argument** source feeds typed bodies. The runner resolves argument
DAGs once per step, with URL encoding and normal report redaction. Deleting a used
argument is disabled until its uses are removed. This extends version two without
changing existing files.

Tests cover all importable project/run fixtures and the workspace, with the frozen
excessive-component-depth fixture checked as a negative import. Another 250
generated documents vary all operators, incomplete operand slots, optional config,
notes and defaults; semantic round trips and print idempotence pass. Identity cases
cover rename, changed kinds, insertion, swaps, ambiguity, cross-cleanup references
and component handles. CLI tests cover both envelopes, no-ops, preserved IDs and
positions, unrelated data, locks, backups/recovery, newer formats and private
errors. Desktop tests click Add through egui pointer events and verify that custom
ports remain visible; runner tests cover all HTTP fields, chained local arguments,
null/false/zero, validation and redaction. This is headless egui evidence, not a
native window acceptance test.

The final Linux workspace run passes all 322 tests, including the 10-test semantic
suite and historical component revisions. Formatting, Clippy with warnings as
errors, both fixture checks and the workspace debug build pass. The process-lock
helper remains the one intentional ignored test, launched by its parent.
No native window or Windows evidence is added here.

## Remaining M4 work

- Broader generated graph topology tests and sustained semantic-lowering fuzzing,
  in addition to the existing syntax fuzzer and generated metadata cases.
- Detailed static type diagnostics beyond the shared document validator; runtime
  input/operator checking remains authoritative for dynamic values.
- Complete acceptance review before merging M4. Desktop code editing/live sync
  stays in M5/M6; CLI execution stays in M6. No native Windows acceptance is claimed.

## Body input regression follow-up

The request inspector now has a multiline JSON/text body editor. Both
`{"productId": "{{input.productId}}"}` and
`{"productId": {{input.productId}}}` produce typed argument leaves; text around a
placeholder produces a template string. The same editor accepts raw text body
templates. Invalid drafts keep the last valid body. Projection is cached between
source changes; complex bindings retain their existing **Body value source**
controls. A separate source-picker fix keeps `input.productId` available inside a
body field named `productId`, while still excluding a custom input's own source.
Tests exercise text entry through egui, invalid draft retention, same-name nested
sources, JSON projection/escaping/limits and the runner's actual request snapshot
for JSON types and text interpolation. No runtime substitution or document version
change was introduced.

The body follow-up passes all 327 workspace tests, formatting, Clippy with warnings
as errors and the workspace debug build. The one ignored process-lock helper is
still launched by its parent. GUI evidence here remains headless egui input tests.
