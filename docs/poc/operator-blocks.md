# Plan and PoC: tests built from operator blocks

Status: implemented in the TypeScript prototype; semantics are ported to Rust in
`apiforge-core::operator` (see [ADR 0001](../decisions/0001-numeric-semantics.md)).

## Authoring model

Requests, comparisons and components remain scenario steps. Each of their arguments can
contain a value, a data source or a nested operator block. The operator's result feeds
the enclosing argument. For example, a Comparison takes the result of
`and(type(status) = text, length(status) > 0)` and expects `true`. Connections to sources
inside blocks remain explicit graph dependencies.

A single Comparison is where a test passes or fails. Comparison and logic operators only
return a boolean. We do not multiply separate "check type", "check length" components.

## Stages

1. A shared operator catalogue: arithmetic, random, comparisons, and/or/not, text, modulo,
   rounding and mathematical functions; plus type, existence, lists and JSON field access.
2. A serialisable expression tree in the existing Binding model; one interpreter for
   simulation and the real API. Validation of nesting, types, references and cycles; no
   eval.
3. An editor of coloured, nested blocks in argument slots. Operators chosen from groups,
   added by clicking or dragging from a palette. Leaves edited with the existing value and
   source editor.
4. Integration with the graph, storage/import, undo, duplication and custom components.
   Example: read the API → compound condition → one Comparison.
5. Tests of the interpreter, type errors, references inside nesting and a full browser
   run.

## PoC rules

The inspiration is the Scratch operator catalogue:
https://github.com/scratchfoundation/scratch-vm/blob/develop/src/blocks/scratch3_operators.js.
We do not copy its implicit type conversion: null and a missing field never become zero,
text never becomes a number, logic takes booleans. API tests need this to detect contract
errors. Equality compares JSON type and structure; text containment is case-sensitive.
Trigonometric angles and the letter index follow the Scratch convention (degrees and
1-based index). Rounding additionally takes a precision. JSON fields and indices keep the
existing paths and 0-based indices.

Operators are a finite catalogue, not JavaScript code. Random is explicit and never a
stable expected value. At most 12 levels and 200 blocks per argument. An export with
operators needs this or a newer application version; an older one rejects the unknown
binding kind.

## Outside the PoC

Loops, waiting/retry, parallelism and a full command-stack editor require extending the
execution and report model. That is the next stage. The PoC covers composing expressions
from blocks in real API scenarios, without turning the engine into an arbitrary script
runner.

## Extension: execution flow (implemented)

1. Model: a manual Start, IF with the same predicates as Comparison, and ordering edges
   with `branch: true/false` (string values). At most one Start per graph, with no inputs;
   the graph stays acyclic.
2. Runner: waits for operations to finish; runs the path from Start and dependent data
   blocks, and marks inactive branches as skipped. IF false is a valid step result that
   selects ELSE. Data can never force a request from a skipped branch to run. Older graphs
   without Start keep their previous semantics.
3. Editor: larger control ports, labelled Next / Yes / No outputs, adding a connected step
   from an output, and editing connections of existing blocks in the configuration. Data
   bindings remain separate ports.
4. Example: product prices → IF status 200 → empty cart → add products → compare the
   total; ELSE reports missing prices, and cleanup deletes only the created cart.
5. Verification: local HTTP measuring ordering and the maximum number of active
   requests; true/false, later steps of both branches, merging, unavailable data, errors,
   cart and cleanup, save/reload and connecting in the browser.
