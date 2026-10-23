# ADR 0005: Scenario language

- Status: accepted
- Date: 2026-10-23
- Scope: new crate `apiforge-script`, `apiforge-cli`, the desktop code panel
- Plan: [scenario code and canvas](../roadmap.md)

## Context

Scenarios and components must be editable as text, equivalent to the board, with
completion and precise diagnostics while the text is incomplete. The same text is
the interface of a future MCP server, so a language model must be able to write it
from a short reference. JSON is too verbose for expressions. YAML puts expressions
in strings and has weak error recovery. The language is therefore our own, small and
expression-oriented, over the document model of [ADR 0004](0004-document-format-v2.md).

## Decision

### Model, not text, is the truth

Text is parsed into a lossless syntax tree, then *lowered* to a **semantic
scenario**: the document model without identity or layout. A separate
*reconciliation* step ([Identity during edits](#identity-during-edits)) turns it
into document changes. The model is printed back by one canonical formatter.

**Semantic equivalence.** `semantic(s)` of a scenario or component is the document
with:

- node `id`s replaced by handles, in node fields, references and ordering edges;
- edge `id`s removed, and edges compared as a set of (source, target, branch);
- nodes compared as a set keyed by handle, so their order does not matter;
- `position`, `pinned` and `proxies` removed;
- component instances naming their definition by component handle and revision
  instead of component id.

Everything else is semantic and must survive the text: kinds, labels, notes,
configuration, bindings in canonical form ([ADR 0004](0004-document-format-v2.md)),
scenario name, description, parameters and error policy, and component inputs,
action and description.

Two properties are tested over generated graphs and every fixture:

1. **Round trip:** `semantic(lower(parse(print(g)))) == semantic(g)`.
2. **Idempotence:** `print(lower(parse(t)))` does not change when printed again.

Identity preservation is a separate contract, tested separately, because the text
deliberately carries no identifiers.

The parser never panics. It always returns a tree with diagnostics, and lowering
skips erroneous statements instead of giving up. Fuzzing enforces this.

### Lexical structure

- UTF-8 source. Whitespace and newlines are insignificant except inside strings.
- `// comment` runs to the end of the line.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Reserved words: `scenario component
  cleanup param input var export start assert condition if else all any exists
  nonempty contains is within with true false null env`. Words that appear only inside
  request blocks (`headers query body text timeout sample contract`) are contextual.
- Numbers use JSON syntax. A request timeout uses a duration: `250ms`, `5s`.
- Strings are `"…"` with JSON escapes. `"""…"""` is a multi-line string whose
  common indentation is removed. Both may interpolate `{{ expression }}`; `\{{`
  writes literal braces.

### Files

A file holds one scenario or one component, matching one tab of the code panel.

```text
file       = scenario | component ;
scenario   = "scenario" STRING "{" scenario_item* "}" ;
scenario_item = "description" STRING
           | "on_error" ("stop" | "continue")
           | "param" key "=" literal
           | statement
           | "cleanup" "{" statement* "}" ;
component  = "component" IDENT STRING "{" component_item* "}" ;
component_item = "description" STRING
           | attribute* "input" key ":" type ("=" literal)?
           | "action" key ( ":" type ("contract" STRING)? )?
           | statement ;
type       = "any" | "string" | "number" | "boolean" | "object" | "array" ;
```

The component's identifier is its `handle`; the string is its display name.
Revision and id are not written. A component lowered without a match gets a new id
at revision 1, and saving a changed definition follows the existing revision rules.

### Statements

```text
statement  = attribute* ( "var" )? IDENT "=" form
           | attribute* "export" IDENT ":" type "=" expr ;
attribute  = "@label" "(" STRING ")"
           | "@after" "(" order ("," order)* ")"
           | "@contract" "(" STRING ")"
           | "@optional" | "@hidden" ;           // on component inputs
order      = IDENT ( "." ("then" | "else") )? ;
form       = "start"
           | METHOD expr request_block?
           | "assert" check
           | "condition" ("all" | "any") "{" check ("," check)* ","? "}" ( "with" expr )?
           | "if" check "{" statement* "}" ( "else" "{" statement* "}" )?
           | IDENT ("@" REVISION)? "(" ( key ":" expr ("," key ":" expr)* ","? )? ")"
           | expr ;
request_block = "{" request_item* "}" ;
request_item  = "argument" key "=" expr
           | "headers" object
           | "query" object
           | "body" ( "text" )? expr
           | "timeout" DURATION
           | "contract" STRING
           | "sample" NUMBER literal ;
check      = expr ( "==" | "!=" | ">" | "<" ) expr
           | expr "~=" expr "within" NUMBER
           | expr "contains" expr
           | expr "is" expr
           | "exists" expr
           | "nonempty" expr
           | expr ;                               // means `expr == true`
```

The form decides the node kind: `start`, an HTTP method (`GET`, `HEAD`, `POST`,
`PUT`, `PATCH`, `DELETE`, `OPTIONS`) is a request, `assert`, `condition` and `if` are
checks, a call of a component handle is an instance, `export` is an Export, and any
other expression is a Value. `var` is allowed only on Values. Lowering reports any
other use as an error.

**Ordering.** `@after(a, b)` writes ordering edges from `a` and `b`, and
`@after(check.then)` writes a branch edge. Statements inside an `if` block receive
the branch edge from that `if` without an attribute. Line order means nothing. The
printer orders statements topologically, keeping the document's node order among
independent statements. A node is printed inside an `if` block exactly when its
only incoming ordering edge is a branch edge of that `if`.

**Start.** `begin = start` is an ordinary statement. As on the board, when a Start
exists only blocks reachable from it run ([ADR 0003](0003-start-reachability.md)).
Without Start, every root runs.

### Expressions

```text
expr     = or ;
or       = and ( "||" and )* ;
and      = equality ( "&&" equality )* ;
equality = relation ( ( "==" | "!=" ) relation )* ;
relation = sum ( ( "<" | ">" | "<=" | ">=" ) sum )* ;
sum      = product ( ( "+" | "-" ) product )* ;
product  = unary ( ( "*" | "/" | "%" ) unary )* ;
unary    = "!" unary | postfix ;
postfix  = primary ( "." IDENT | "[" ( NUMBER | STRING ) "]" )* ;
primary  = literal | STRING | IDENT | call | "?"
         | "env" "." IDENT | "env" "[" STRING "]"
         | "param" "." IDENT | "param" "[" STRING "]"
         | "input" "." IDENT | "input" "[" STRING "]"
         | "{" ( key ":" expr ( "," key ":" expr )* ","? )? "}"
         | "[" ( expr ( "," expr )* ","? )? "]"
         | "(" expr ")" ;
literal  = NUMBER | "-" NUMBER | STRING_WITHOUT_INTERPOLATION | "true" | "false" | "null"
         | object or array of literals ;
key      = IDENT | STRING ;
call     = (IDENT | "op" "." IDENT) ("@" REVISION)? "(" arguments? ")" ;
arguments = expr ("," expr)* ","? | key ":" expr ("," key ":" expr)* ","? ;
```

| Syntax | Binding |
| --- | --- |
| `+ - * / %`, `== != > < >= <=`, `&& \|\| !` | `expression` with Add, Subtract, Multiply, Divide, Modulo, Equals, NotEquals, Greater, Less, AtLeast, AtMost, And, Or, Not |
| `round(x, 2)`, `sqrt(x)`, `concat(a, b)`, … | `expression`, one function per remaining operator, named after its identifier in the catalogue |
| `"text {{x}}"` | `template`; without interpolation a string `literal` |
| `{ a: x }`, `[x, y]` | always `object` / `array`, also when every leaf is a literal |
| `env.name` | `environment` |
| `param.name` | `parameter` |
| `input.name.path` | local request `argument` |
| `handle.path` | `reference` |

Unary minus applies only to number literals, because no operator negates a value.
The syntax formatter preserves explicit parentheses. The document printer adds
parentheses where precedence needs them.

Parameter and component input names may be quoted to represent existing document
keys that are not identifiers. The same applies to named call arguments. `?`
represents an unconnected binding slot, distinct from a literal `null`; lowering
may only omit it in a position that the document model permits to be absent.
`op.add(left: value)` explicitly names an operator and its input slots, including
incomplete operators. Ordinary function calls may use positional arguments;
positional and named arguments cannot be mixed. Component revisions are positive
integers. Revision qualifiers on operators are rejected during lowering.

**References.** The identifier is a handle in scope. If the node has a single
output port, the following segments are the path into it, so `first.price` reads
`price` of a Value. Otherwise the first segment names the port: `products.status`,
`products.body.items[0]`, `ok.passed`. The printer writes paths in canonical form:
`.name` for identifier keys, `[0]` for digit keys, and `["any key"]` otherwise.

### Checks

`assert`, `if` and each `condition` entry hold one predicate: `==` Equals, `!=`
NotEquals, `>` Greater, `<` Less, `~= … within t` Approximately with tolerance,
`contains`, `is` Type, `exists`, `nonempty`. `condition … with expr` sets the data
the condition passes on (its `value` input). A check that is a bare expression
means `expr == true`. The printer writes it in that full form, so `assert a >= b`
prints as `assert (a >= b) == true`. Inside a check, an operand that is itself a
comparison is parenthesised. Condition entry *n* lowers to inputs `actual<n>` and
`expected<n>`.

### Requests

```text
order = POST "/orders/{{cart.id}}" {
  headers { Accept: "application/json" }
  query { limit: 10 }
  body { productId: first.id, total: total }
  timeout 5s
  sample 201 { "id": "o-1" }
}
```

The URL expression lowers to `inputs.url`, `headers` and `query` to object
bindings, and `body` to `inputs.body` with `bodyFormat: json`. `body text …`
selects `bodyFormat: text`. The printer omits defaults: empty headers and query,
no body, a 10 s timeout, no sample.

### Comments and labels

Consecutive `//` lines directly above a statement, and a comment at the end of
its line, become the node's `note`. The printer writes notes back as line comments
above the statement. Any other comment produces a warning that the formatter will
drop it. `@label("…")` sets the display label.

### Exact document metadata

The first semantic implementation exposed fields that the shorthand could not
retain: absent versus explicitly default configuration values, optional input
flags, action metadata independent of the input, whitespace in notes and extra
input bindings permitted by the document model. The following spellings preserve
them without writing internal identities or layout:

- `@config({ ... })` replaces a block's complete `NodeConfig`, excluding
  `componentId` and `revision` (which come only from the component call). This is
  literal JSON with the document's field names. Unknown fields are errors. It is
  emitted only when the shorthand would change configuration presence or values.
  It never silently overrides the statement: every setting the shorthand writes
  (method, timeout, sample, predicate, tolerance, body format, `var`, export type,
  condition mode, contract) must appear in `@config` with the same value, or be
  absent there while the shorthand writes its default. Otherwise lowering
  reports an error at `@config`, so editing `timeout 10s` beside it cannot be
  ignored.
- `@inputs({ name: expression, ... })` replaces the complete input map when the
  block shorthand cannot carry all its slots. Bindings use ordinary expressions,
  never raw node IDs. It does not bypass document validation.
- `@note("...")` preserves an exact note that trimmed line comments cannot carry.
- Component inputs support `@required(true|false)` and `@exposed(true|false)`;
  `@optional` and `@hidden` remain the short forms for false.
- `action key: type contract "id"` specifies action metadata independently of the
  input's type/contract. Without a type, the action inherits the input metadata.
  An explicit type without `contract` means an absent action contract.

Named request arguments are declared with `argument key = expression` inside a
request block. They are used as `input.key` in expressions and templates; each
request has its own scope. HTTP field names are reserved. Dependencies among
arguments are acyclic; declaration order has no meaning. See
[ADR 0004's follow-up](0004-document-format-v2.md#named-request-arguments-follow-up-23-october-2026).

Printing an existing editor document can retain an intentionally unresolved
environment name. `LowerContext::document` supports this projection;
`LowerContext::new` and CLI `check`/`apply` require workspace name resolution.
No environment or secret values are substituted into source.

### Identity during edits

Reconciliation applies a semantic scenario to the current document. It never
guesses: when the text is ambiguous, a new block is safer than a block that
silently inherits another one's identity, position and run history.

1. A node whose handle and kind are unchanged keeps its `id`, `position` and
   `pinned`, whatever else changed.
2. **Unambiguous rename.** Exactly one node disappears and exactly one appears
   within one synchronisation. They have the same kind and are equal apart from
   the handle, and every use of the old handle now names the new one. The new
   node keeps the old identity. Any other combination, for example a removed
   request and a different request written in its place, creates a new node.
3. Every other new statement creates a node with a fresh id and no position.
4. A statement that keeps a handle but changes the kind (e.g. a Value rewritten
   as a request) creates a new node; the old one is removed.

**Explicit rename.** The editor's rename action (F2 in code, the handle field in
the inspector) renames the handle in the document first, updates every use, then
reprints. It always keeps identity and never depends on rule 2.

Components follow the same rules by component handle. Reconciliation is tested
against the cases above, including insert-in-place and swap of two statements,
which must not exchange identities.

### Command line

A `.afs` file names components, environment variables and parameters, but holds
neither their definitions nor environment values. The CLI therefore always works
against a workspace or project document (the version-2 JSON of storage and
export):

```text
apiforge fmt <file.afs>                      syntax only; formats in place or --check
apiforge check --workspace <doc.json> [--scenario <name|id>] [<file.afs>]
apiforge print --workspace <doc.json> --scenario <name|id>      document → text
apiforge apply --workspace <doc.json> --scenario <name|id> <file.afs>
apiforge run   --workspace <doc.json> --scenario <name|id> [<file.afs>]
               [--mode simulation|live] [--base-url <url>] [--secrets <file>]
```

- `check` resolves handles, components (by handle, latest revision unless
  `Name@revision`), contracts, parameters and environment names against the
  document. Without `<file.afs>` it checks the scenario stored in the document.
- `apply` validates, reconciles and writes the document atomically with a backup,
  using the storage rules ([ADR 0002](0002-workspace-storage.md)).
- `run` runs the stored scenario, or the given text applied in memory without
  writing the document. The base URL comes from the document's environment unless
  overridden. Secret values never appear on the command line: they come from
  `APIFORGE_SECRET_<NAME>` environment variables or a `--secrets` file of
  `NAME=value` lines, and are redacted like in the application.
- Exit codes: 0 success or passed run, 1 diagnostics, document errors or a failed
  run, 2 invalid arguments.

A git-friendly project layout of `.afs` files remains an open question; it would
add a project manifest, not change these commands.

### API

`apiforge-script` exposes, without GUI dependencies:

```rust
pub fn parse(text: &str) -> SyntaxTree;                       // lossless, never fails
pub fn lower(tree: &SyntaxTree, context: &LowerContext) -> Lowered; // semantic scenario + diagnostics
pub fn reconcile(current: &Graph, lowered: &Lowered) -> Reconciled; // document changes, identities kept
pub fn print_scenario(project: &Project, scenario: &Scenario) -> String;
pub fn print_component(project: &Project, component: &Component) -> String;
pub fn complete(tree: &SyntaxTree, offset: usize, context: &CompletionContext) -> Vec<Completion>;
pub fn hover(tree: &SyntaxTree, offset: usize, context: &CompletionContext) -> Option<Hover>;
```

A `Diagnostic` carries a byte range, a severity, a message and optional fixes.
Diagnostics never quote secret values.

## Consequences

- One formatter means no style debates and stable diffs. User formatting is not
  preserved; comments survive only as notes.
- Everything the board can express is expressible in text and vice versa,
  including ordering, branches and Start. Layout is the only exception.
- The language reference for users and MCP is generated from this ADR in M7. Any
  grammar change updates this ADR, the reference and the round-trip corpus
  together.
