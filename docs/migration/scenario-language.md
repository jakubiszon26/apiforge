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

`apiforge-cli` currently exposes only:

```bash
cargo run -p apiforge-cli --locked -- fmt fixtures/scripts/checkout.afs --check
cargo run -p apiforge-cli --locked -- fmt path/to/scenario.afs
```

`--check` can precede or follow the filename. It never writes. Normal formatting
uses the storage crate's synced temporary-file/atomic-rename implementation and
avoids a write when the file is already formatted. Invalid syntax, invalid UTF-8
and oversized input/output leave the source untouched. Warnings and errors go to
stderr without source excerpts. Exit codes are 0 for success, 1 for diagnostics,
I/O or a formatting mismatch, and 2 for invalid arguments.

The checked-in [scenario](../../fixtures/scripts/checkout.afs) and
[component](../../fixtures/scripts/lookup.afs) examples are canonical syntax
fixtures. They are not runnable fixtures yet.

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

## Remaining M4 work

1. Lower syntax into the document model against workspace context: handle scopes,
   components and revisions, operators/slots, parameters, environments, contracts,
   type rules, data dependencies and explicit/implicit branch ordering. Unknown
   references and attributes currently parse; `fmt` does not validate them.
2. Print documents without losing any semantic fields, including optional config
   values, missing bindings, notes, component inputs/actions and cleanup. Resolve
   any grammar gaps before asserting document/text equivalence.
3. Reconcile identity and layout by the ADR 0005 rules; cover a single unambiguous
   rename, changed kinds, insertion, swaps, ambiguous edits and references.
4. Add workspace-aware `check`, `print` and `apply`. `apply` must use S1's exclusive
   session and external-change checks, retain the project/workspace envelope and
   follow deliberate recovery rules.
5. Establish graph/document semantic round trips over generated graphs and all
   compatibility fixtures. Merge M4 only after its complete acceptance criteria
   pass. Desktop editor work stays in M5/M6 and CLI execution stays in M6.

No desktop code or persistence format changes in this syntax checkpoint.
