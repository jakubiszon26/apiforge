# ADR 0005: Scenario language

- Status: accepted
- Date: 2026-10-23
- Scope: new crate `apiforge-script`, `apiforge-cli`, the desktop code panel
- Plan: [scenario code and canvas](../plans/code-and-canvas.md)

## Context

Scenarios and components must be editable as text, equivalent to the board, with
completion and precise diagnostics while the text is incomplete. The same text is
the interface of a future MCP server, so a language model must be able to write it
from a short reference. JSON is too verbose for expressions. YAML puts expressions
in strings and has weak error recovery. The language is therefore our own, small and
expression-oriented, over the document model of [ADR 0004](0004-document-format-v2.md).

## Decision

### Model, not text, is the truth

Text is parsed into a lossless syntax tree, then *lowered* onto the document model.
The model is printed back by one canonical formatter. Two properties are tested
over generated graphs and every fixture:

1. **Round trip:** `lower(parse(print(g))) == g`, ignoring layout.
2. **Idempotence:** `print(lower(parse(t)))` does not change when printed again.

The parser never panics. It always returns a tree with diagnostics, and lowering
skips erroneous statements instead of giving up. Fuzzing enforces this.

### Lexical structure

- UTF-8 source. Whitespace and newlines are insignificant except inside strings.
- `// comment` runs to the end of the line.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Reserved words: `scenario component
  cleanup param input var export start assert condition if else all any exists
  nonempty contains is within true false null env`. Words that appear only inside
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
           | "param" IDENT "=" literal
           | statement
           | "cleanup" "{" statement* "}" ;
component  = "component" IDENT STRING "{" component_item* "}" ;
component_item = "description" STRING
           | attribute* "input" IDENT ":" type ("=" literal)?
           | "action" IDENT
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
           | "condition" ("all" | "any") "{" check ("," check)* ","? "}"
           | "if" check "{" statement* "}" ( "else" "{" statement* "}" )?
           | IDENT "(" ( IDENT ":" expr ("," IDENT ":" expr)* ","? )? ")"
           | expr ;
request_block = "{" request_item* "}" ;
request_item  = "headers" object
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
primary  = literal | STRING | IDENT | IDENT "(" ( expr ( "," expr )* )? ")"
         | "env" "." IDENT | "env" "[" STRING "]"
         | "param" "." IDENT
         | "{" ( key ":" expr ( "," key ":" expr )* ","? )? "}"
         | "[" ( expr ( "," expr )* ","? )? "]"
         | "(" expr ")" ;
literal  = NUMBER | "-" NUMBER | STRING_WITHOUT_INTERPOLATION | "true" | "false" | "null"
         | object or array of literals ;
key      = IDENT | STRING ;
```

| Syntax | Binding |
| --- | --- |
| `+ - * / %`, `== != > < >= <=`, `&& \|\| !` | `expression` with Add, Subtract, Multiply, Divide, Modulo, Equals, NotEquals, Greater, Less, AtLeast, AtMost, And, Or, Not |
| `round(x, 2)`, `sqrt(x)`, `concat(a, b)`, … | `expression`, one function per remaining operator, named after its identifier in the catalogue |
| `"text {{x}}"` | `template`; without interpolation a string `literal` |
| `{ a: x }`, `[x, y]` | `object` / `array`; with only literal leaves a `literal` |
| `env.name` | `environment` |
| `param.name` | `parameter` |
| `handle.path` | `reference` |

Unary minus applies only to number literals, because no operator negates a value.
Parentheses are printed only where precedence needs them.

**References.** The identifier is a handle in scope. If the node has a single
output port, the following segments are the path into it, so `first.price` reads
`price` of a Value. Otherwise the first segment names the port: `products.status`,
`products.body.items[0]`, `ok.passed`. The printer writes paths in canonical form:
`.name` for identifier keys, `[0]` for digit keys, and `["any key"]` otherwise.

### Checks

`assert`, `if` and each `condition` entry hold one predicate: `==` Equals, `!=`
NotEquals, `>` Greater, `<` Less, `~= … within t` Approximately with tolerance,
`contains`, `is` Type, `exists`, `nonempty`. A check that is a bare expression
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

### Identity during edits

Lowering reconciles with the current graph instead of creating nodes afresh:

1. A statement whose handle matches a node keeps that node's `id`, `position`
   and `pinned`.
2. An unmatched statement that takes the place of an unmatched node of the same
   kind is a rename of that node. *The same place* means the same index among
   statements of that kind.
3. Otherwise a new node is created with a fresh id and no position.

The same rules apply to components by handle. The editor's explicit rename (F2)
renames a handle and every use, and never depends on the heuristic.

### API

`apiforge-script` exposes, without GUI dependencies:

```rust
pub fn parse(text: &str) -> SyntaxTree;                       // lossless, never fails
pub fn lower(tree: &SyntaxTree, context: &LowerContext) -> Lowered; // document + diagnostics
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
