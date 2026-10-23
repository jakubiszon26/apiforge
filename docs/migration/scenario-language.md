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

## Generated topologies, semantic fuzzing and diagnostics

`tests/topology.rs` generates 400 random scenario DAGs (about 5000 blocks) from
every statement form: Values and variables, requests with arguments, headers,
query and bodies, `assert`, `condition`, nested IF branches, `@after` with
`.then`/`.else`, component instances and cleanup. Each one must lower and
validate; print -> lower must keep its semantics and print identically again;
reapplying the printed source must keep every node ID, position and edge ID; a
complete rename of any handle must keep identity; and an inserted statement
must be the only node needing layout. The shared generator lives in
`tests/support/generator.rs`.

The test found that the printer filtered one global topological order per IF
block. A branch statement reading a later outer statement made each print choose
a different order. Each block is now ordered by the dependencies of its
members' whole subtrees; a greedy stable choice makes print -> lower -> print a
fixed point.

The dependency-free `fuzz_semantics` example mutates generated sources by
lines and tokens (deletion, duplication, swaps, handle substitution and
fragments such as `@after`, `if`, `cleanup`, `argument`, `@inputs`). It checks
diagnostic ranges, that failed lowering never reconciles, and that every
successful lowering prints, round-trips, prints identically and reconciles
without dangling references:

```bash
cargo run -p apiforge-script --example fuzz_semantics --release --locked -- 60 2718281828
```

It found that the printer wrote undeclared component instance inputs as call
arguments, which lowering rejects; they now use `@inputs`.
On Linux, release runs found no further failure: seeds `1592639710`,
`31415926` and `424242` for 120 seconds each (1,156,429 cases, 176,681 of them
lowered without errors), then `2718281828` for 300 seconds (914,606 cases,
139,625 valid) on the code with type warnings. A final 180-second run with seed
`1592639710` on the code including the `@config` correction below completed
552,135 cases, 83,931 valid. The syntax fuzzer passed 30 more seconds
(5,673,127 mutations).

Document validation errors (for example Start inside a branch or `@after` on an
IF without `.then`/`.else`) now point at the statement instead of the whole
file. Edge errors are reported at their target statement.

Lowering adds static type warnings that mirror the board's connection rule
(`graph::compatible`): a whole output must be compatible with the slot it fills,
including contracts, and literals, operators, templates, objects and arrays
must have the slot's base type. Warnings point at the operand or input. They are
not errors because the document model accepts such bindings and runtime checks
remain authoritative. Every stored fixture document lowers without warnings.

A manual CLI acceptance pass over a copy of `fixtures/workspace.json` found that
`@config` silently overrode visible shorthand. The printed cleanup request carries
`@config` (its sample status is absent) next to `timeout 10s`; changing it to
`3s` made `apply` exit 0 without changing `timeoutMs`. Every setting the
shorthand writes must now agree with `@config` or be absent there while the
shorthand writes its default; otherwise lowering reports an error at `@config`
and `apply` preserves the document (exit 1). A no-op `apply` of the printed
scenario still leaves the file and backup untouched.

The Linux workspace run passes all 334 tests, including the topology test and
16 semantic tests. Formatting, Clippy with warnings as errors, both fixture checks,
`fmt --check` on both example scripts and `check` on the workspace fixture pass.
The process-lock helper remains the one intentional ignored test. No native
window or Windows evidence is added.

## Remaining M4 work

- Acceptance review before merging M4 into `main`. Desktop code editing and live
  sync stay in M5/M6; CLI execution stays in M6. No native Windows acceptance is
  claimed.
