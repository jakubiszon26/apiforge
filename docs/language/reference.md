Reference version: 1 · grammar of ADR 0005 as of 2026-10-27

# The .afs language

An `.afs` file describes one API test **scenario** or one reusable
**component** of API Forge. The same text is the board of the desktop
application: every statement is one block, and the board and the text are two
views of one document. This page is everything you need to write a correct
file. It is checked by a test of the repository
(`crates/apiforge-project/tests/reference.rs`): every `afs` example below
parses, is in the canonical form the formatter writes, and the complete
component and scenario at the end lower against a real project.

The grammar is decided in [ADR 0005](../decisions/0005-scenario-language.md);
where this page and the parser of the binary you are using disagree, the
parser wins and the page has a defect.

## How to read a file

- A file holds exactly one `scenario` or one `component`.
- Inside it, every **statement** has the form `handle = form`. The handle names
  the block; the form says what it does (a request, a check, a value, …).
- **Line order means nothing.** A statement runs after the statements whose
  values it uses and after those named in its `@after(…)` attribute. Write
  statements in a readable order; the formatter keeps the order you wrote,
  and the window, printing from the board, puts each statement after those it
  uses.
- Whitespace and line breaks are insignificant outside strings. There is one
  canonical layout (two-space indentation, one statement per line, one
  attribute per line), and the formatter (`apiforge fmt`) writes it. The
  examples below are in that layout.
- Identifiers are `[A-Za-z_][A-Za-z0-9_]*`. A handle is an identifier of at
  most 64 characters, unique in its file (main flow and cleanup together), and
  not one of the reserved words: `scenario component cleanup param input var
  export start assert condition if else all any exists nonempty contains is
  within with true false null env GET HEAD POST PUT PATCH DELETE OPTIONS`.

## File shape

A scenario has a quoted name; a component has a handle (how other files call
it) and a quoted display name.

```afs
scenario "Smoke test" {
  description "Checks that the shop answers."
  on_error continue
  ping = GET "/products/123" {
    sample 200 { id: 123 }
  }
  answered = assert ping.status == 200
}
```

```afs
component ping_shop "Ping the shop" {
  description "One request and its status."
  ping = GET "/products/123" {
    sample 200 { id: 123 }
  }
  export status: number = ping.status
}
```

- `description "…"` is optional, at most once.
- `on_error stop` or `on_error continue` (scenarios only). With `stop`, the
  default, the first failed assertion, error or blocked step stops the main
  flow: the remaining steps are skipped and the cleanup still runs. With
  `continue` every step that can still run does, and the run reports every
  failure.
- The formatter writes the parts in this order: `description`, `on_error`,
  `param` lines, `input` lines, `action`, statements, `cleanup`. Write them in
  that order.
- A component has no `on_error`, `param` or `cleanup`; a scenario has no
  `input`, `action` or `export`.

## Parameters and inputs

A scenario declares **parameters** with a literal default. Every statement
reads them as `param.name`. A key that is not an identifier is quoted, and
read with brackets.

```afs
scenario "Parameters" {
  param productId = 123
  param "page size" = 20
  param filter = { category: "coffee", inStock: true }
  page = GET "/products" {
    query { size: param["page size"], category: param.filter.category }
    sample 200 { items: [] }
  }
}
```

A component declares **inputs** with a type, `any`, `string`, `number`,
`boolean`, `object` or `array`, and optionally a literal default. Inside the
component, an input is read with the same `param.name` syntax.

```afs
component add_to_cart "Add to cart" {
  @label("Product ID")
  input productId: number
  @optional
  input quantity: number = 1
  action productId
  added = POST "/carts" {
    body { productId: param.productId, quantity: param.quantity }
    sample 201 { id: "c-1" }
  }
  export cart: string = added.body.id
}
```

- Defaults and parameter values are literal JSON: numbers, strings without
  `{{…}}`, `true`, `false`, `null`, objects and arrays of those. They cannot
  refer to anything.
- `@label("…")` sets the name the window shows for the input.
- `@optional` (the same as `@required(false)`) lets a caller leave the input
  out; the run dialog then allows it to stay empty. An input with a default
  gets its default when left out. One without a default has no value at all,
  not `null`: `exists param.note` is false, and a step that needs the value
  fails.
- `@hidden` (the same as `@exposed(false)`) hides the input from callers on
  the board. `@contract("…")` gives it a contract of the project.
- `action name` names the input the board connects when the component is
  dropped after another block's output. It changes nothing in a run.

## Start and ordering

`begin = start` declares the start of the main flow. **When a `start`
exists, a statement runs only if it follows the start**: through `@after`, or
by using the value of a statement that does. Values those statements need are
computed as well. Without a `start`, every statement runs. A file has at most
one `start`.

`@after(a, b)` writes ordering: the statement runs after `a` and `b` have run,
without using their values. A statement that uses another's value is already
ordered after it and needs no `@after`.

```afs
scenario "Ordering" {
  begin = start
  @after(begin)
  catalogue = GET "/products" {
    sample 200 { items: [] }
  }
  @after(catalogue)
  health = HEAD "/health" {
    sample 200 {}
  }
  @after(begin, health)
  allowed = OPTIONS "/carts" {
    sample 204 {}
  }
  // Nothing orders this request after begin, so it never runs.
  unused = GET "/unused" {
    sample 200 {}
  }
}
```

`@after(check.then)` and `@after(check.else)` order a statement after one
branch of an `if` (see [Branches](#branches-if-and-else)).

## Requests

A request is an HTTP method followed by the URL and an optional block of
fields. The methods are `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE` and
`OPTIONS`.

```afs
scenario "Requests" {
  description "Every request field."
  on_error continue
  products = GET "/products" {
    headers { Accept: "application/json", "X-Trace": "reference" }
    query { limit: 10, sort: "price" }
    timeout 5s
    contract "Product list"
    sample 200 { items: [{ id: 123, price: 100 }] }
  }
  created = POST "https://shop.example.com/carts" {
    body { productId: 123, quantity: 2 }
    sample 201 { id: "c-1" }
  }
  renamed = PATCH "/carts/{{input.cart}}" {
    argument cart = created.body.id
    body { note: "gift" }
    timeout 250ms
    sample 200 { id: "c-1", note: "gift" }
  }
  noted = PUT "/carts/{{created.body.id}}/note" {
    headers { "Content-Type": "text/plain" }
    body text "Leave at the door"
    sample 204 {}
  }
  removed = DELETE "/carts/{{created.body.id}}" {
    sample 200 { deleted: true }
  }
}
```

- **URL.** A string, usually a template. A URL that does not start with
  `http://` or `https://` is joined to the project's base URL, so
  `"/products"` is the usual form. Values interpolated into a URL with
  `{{…}}` are URL-encoded.
- **`headers { … }` and `query { … }`** are objects; each value is sent as
  text. Quote a name that is not an identifier (`"X-Trace"`).
- **`body expr`** sends JSON. **`body text expr`** sends the text as it is.
- **`timeout`** is a duration written without a space: `250ms`, `5s`. It must
  be more than 0 and at most 60 s. Without it the timeout is 10 s.
- **`sample STATUS literal`** is the response a **simulation** uses instead of
  sending the request. A simulation of a request without a sample fails, so
  give every request one. The status is 100 to 599 and the body is literal
  JSON.
- **`contract "…"`** names a contract of the project (by id, or by a unique
  name) that describes the response body.
- **`argument name = expr`** declares a named value of this request, read as
  `input.name` in this request's own URL, headers, query and body. The names
  `url`, `headers`, `query` and `body` are taken.
- Each field appears at most once (`argument` once per name). The formatter
  keeps them in the order written.

A request has four outputs, and a reference to it must name one:
`products.status` (number), `products.body` (the parsed response),
`products.headers` (object) and `products.duration` (number, milliseconds).
After the output comes a path into it: `products.body.items[0].price`.

## Expressions and operators

Expressions compute values from literals, parameters, environment values and
the outputs of other statements.

```afs
scenario "Expressions" {
  param price = 19.99
  param count = 3
  param tags = ["coffee", "beans"]
  subtotal = param.price * param.count
  shipping = 5 + subtotal / 100
  change = 50 - subtotal % 10
  rounded = round(subtotal, 2)
  cheap = subtotal <= 50 || param.count >= 10
  expensive = subtotal > 100 && param.count < 5
  dear = !cheap
  same = param.count == 3 && param.price != 0
  first_tag = param.tags[0]
  title = "{{param.count}} items for {{rounded}}"
  summary = { total: rounded, cheap: cheap, tags: param.tags }
  label = concat("Total: ", rounded)
  size = length(param.tags)
  base = env.baseUrl
  token = env["api-token"]
}
```

**Names.** An expression starts at one of:

| Written | Means |
| --- | --- |
| `handle` or `handle.output.path` | the output of another statement |
| `param.name` | a scenario parameter, or a component input |
| `env.name`, `env["name"]` | an environment value of the project (text); `env.baseUrl` always exists |
| `input.name` | an `argument` of the request being written |

A secret is used the same way, by its name: `env.apiToken`. Never write a
secret's value into a file.

**Paths.** `.name` reads a field, `[0]` an array item, `["any key"]` a field
whose name is not an identifier, and `.length` at the end of a path into an
array its length. A statement with a single output (a value, a component with
one export) is read directly: `first.price` reads `price` of
the value `first`. A statement with several outputs names the output first:
`products.body.items[0]`, `ok.passed`.

**Literals.** JSON numbers (`3`, `-1.5`), strings `"…"` with JSON escapes,
`true`, `false`, `null`, arrays `[a, b]` and objects `{ key: value }`.
Object keys are identifiers or strings. A string containing `{{expression}}`
is a template; `\{{` writes literal braces. A triple-quoted string
`"""…"""` spans lines and loses its common indentation; the formatter
rewrites it as an ordinary string with `\n`.

**Operators**, from the loosest to the tightest binding; all are
left-associative:

| Operators | Meaning |
| --- | --- |
| `\|\|` | or |
| `&&` | and |
| `==` `!=` | equal, not equal (strict JSON equality, no conversion) |
| `<` `>` `<=` `>=` | numeric comparison |
| `+` `-` | add, subtract (numbers only) |
| `*` `/` `%` | multiply, divide, modulo (the sign follows the divisor) |
| `!` | not |
| `.name` `[0]` `["key"]` | path |

Parentheses group. `-` directly before a number literal makes it negative;
nothing negates a computed value, so write `0 - x`. `&&` and `||` evaluate
both sides. Arithmetic never converts text to numbers; `+` does not join text
(use `concat`).

**Functions** take positional arguments, `round(x, 2)`, or the operand names
`left` and `right`: `op.round(left: x, right: 2)`. Both mean the same; the
window prints the second form when it writes a file from the board. Do not
mix positional and named arguments in one call.

| Function | Result |
| --- | --- |
| `add` `subtract` `multiply` `divide` `modulo` (`a, b`) | the operators above, as functions |
| `equals` `notEquals` `greater` `less` `atLeast` `atMost` (`a, b`) | comparisons |
| `and` `or` (`a, b`), `not` (`a`) | logic |
| `round(x, places)` | `x` rounded to `places` decimals, 0 to 12 |
| `random(from, to)` | a random number between the two; a whole number when both are whole |
| `abs` `floor` `ceiling` `sqrt` `ln` `log` `exp` `power10` (`x`) | number functions; `log` is base 10 |
| `sin` `cos` `tan` (`degrees`), `asin` `acos` `atan` (`x`) | trigonometry; angles in and out are in degrees |
| `concat(a, b)` | the two values joined as text |
| `letter(text, position)` | the letter at `position`, counted from 1 |
| `length(x)` | the length of a text or an array |
| `contains(x, value)` | whether a text contains a text, or an array an item |
| `typeOf(x)` | `"string"`, `"number"`, `"boolean"`, `"object"`, `"array"` or `"null"` |
| `exists(x)` | whether `x` has a value (`null` counts as a value) |
| `field(data, "items[0].price")` | the value at a path into `data` |

A function call cannot hold a component call. Function arguments may be any
expression.

## Values and var

A statement whose form is an expression is a **value** block: it computes once
and its output is the value.

```afs
scenario "Values" {
  limit = 10
  var greeting = "Hello"
  settings = { limit: limit, language: "en" }
  ids = [1, 2, 3]
  message = "{{greeting}}, the limit is {{settings.limit}}"
  nothing = null
}
```

`var` marks a value as a **variable**: the window lists it in its Variables
panel and lets the board show copies of it. It changes nothing in a run, and
is allowed only on value blocks.

## Assertions and predicates

`assert` checks one predicate. A failed assertion fails the step (and with
`on_error stop`, stops the main flow). Every predicate:

```afs
scenario "Assertions" {
  param expected = 160
  cart = GET "/carts/c-1" {
    sample 200 { id: "c-1", total: 160.004, items: [{ productId: 123 }], note: "" }
  }
  ok = assert cart.status == 200
  not_missing = assert cart.status != 404
  positive = assert cart.body.total > 0
  small = assert cart.body.total < 1000
  close = assert cart.body.total ~= param.expected within 0.01
  has_item = assert cart.body.items contains { productId: 123 }
  named = assert cart.body.id contains "c-"
  typed = assert cart.body.total is "number"
  present = assert exists cart.body.note
  filled = assert nonempty cart.body.items
  at_least = assert (cart.body.total >= 100) == true
  valid = condition all {
    cart.status == 200,
    nonempty cart.body.items,
  } with cart.body
  either = condition any {
    cart.body.total > 100,
    exists cart.body.coupon,
  }
}
```

| Predicate | Passes when |
| --- | --- |
| `a == b`, `a != b` | the values are (not) strictly equal as JSON |
| `a > b`, `a < b` | both are numbers and the comparison holds; anything else is an error |
| `a ~= b within t` | both are numbers and differ by at most `t`, a number literal |
| `a contains b` | text `a` contains text `b`, or array `a` has an item equal to `b` |
| `a is "type"` | the type of `a` is `"string"`, `"number"`, `"boolean"`, `"object"`, `"array"` or `"null"` |
| `exists a` | `a` has a value; `null` is a value, a missing field is not |
| `nonempty a` | `a` exists and is not `null`, `""`, `[]` or `{}` |

- There is no `>=` or `<=` predicate. A check that is a plain expression
  means `expr == true`, and the formatter writes it in full:
  `assert a >= b` becomes `assert (a >= b) == true`, `assert flag` becomes
  `assert flag == true`.
- An operand that is itself a comparison is parenthesised:
  `assert (a == b) == true`.
- An assertion has two outputs: `ok.passed` (boolean) and `ok.value` (the
  checked value).
- **`condition all { … }` / `condition any { … }`** holds several checks,
  separated by commas, and passes when all (any) hold. It guards data rather
  than reporting a failure: when it does not hold, the step is *blocked* and
  every step that uses it is blocked too. `with expr` sets the value it passes
  on (`valid.value`); without `with`, it passes on its first check's value. All
  `~=` checks of one condition share one tolerance.
- In a check, `exists` and `contains` are predicates. Write the function form
  as `op.exists(x)` or `op.contains(x, y)` there.

## Branches: if and else

`if` checks one predicate, like `assert`, and chooses a branch. It never fails.
The statements inside its braces run only on that branch.

```afs
scenario "Branches" {
  param quantity = 2
  cart = POST "/carts" {
    body { productId: 123, quantity: param.quantity }
    sample 201 { id: "c-1", total: 160 }
  }
  created = if cart.status == 201 {
    receipt = GET "/carts/{{cart.body.id}}" {
      sample 200 { id: "c-1" }
    }
  } else {
    failed = assert cart.status == 201
  }
  @after(created.then, receipt)
  thanks = "Cart {{cart.body.id}} is ready"
}
```

- `else { … }` is optional. Statements in a branch may use values from
  outside it; their handles are unique in the whole file.
- `@after(created.then)` orders a statement outside the braces after the
  `then` branch; `.else` after the other. A statement whose only ordering is
  one branch is printed inside that branch's braces by the window.
- `if` has the outputs `created.passed` and `created.value`, like `assert`.

## Cleanup

A scenario's `cleanup { … }` block runs after the main flow, whatever its
outcome, and is meant to remove what the test created. Its statements may use
values of the main flow; the main flow cannot use them. A cleanup step whose
input never came into being (the request that would have created it did not
run) is skipped.

```afs
scenario "Cleanup" {
  cart = POST "/carts" {
    body { productId: 123, quantity: 1 }
    sample 201 { id: "c-1" }
  }
  cleanup {
    removed = DELETE "/carts/{{cart.body.id}}" {
      sample 200 { deleted: true }
    }
    gone = assert removed.body.deleted == true
  }
}
```

`@after` in the cleanup names only cleanup statements. A `start` in the main
flow does not govern the cleanup: every cleanup statement runs.

## Exports

`export name: type = expr` defines an output of a component. Exports exist
only in components; a caller reads them by name. The type is one of the input
types.

```afs
component product_price "Product price" {
  input productId: number
  product = GET "/products/{{param.productId}}" {
    sample 200 { id: 123, price: 100, promotion: { active: true, price: 80 } }
  }
  export price: number = product.body.promotion.price
  export regular: number = product.body.price
}
```

The handle of an export is the output's name, so it is unique like any other
handle.

## Component calls

A statement calls a published component by its handle and revision:
`Handle@revision(input: value, …)`. The arguments are named after the
component's inputs, in any order; inputs you leave out get their default or no
value.

```afs
scenario "Component calls" {
  param productId = 123
  begin = start
  @after(begin)
  pricing = product_price@1(productId: param.productId)
  saving = pricing.regular - pricing.price
  cheaper = assert saving > 0
  @after(cheaper)
  status = ping_shop@2()
  answered = assert status == 200
}
```

- **Always write the revision.** In a project folder a call without
  `@revision` is an error, and the error's fix inserts the latest published
  revision. Only published revisions can be called; a component you have
  just written must be published first. A newer revision never changes an
  existing call: you change the number.
- The outputs are the component's exports: `pricing.price`. A component with
  exactly one export has a single output, read directly: `status` above is
  `ping_shop`'s `status` export, and a path continues into it.
- A call is a whole statement. It cannot appear inside another expression;
  give it its own handle and use that.
- A call that fails inside (a failed assertion, an error) fails or blocks the
  calling step.

## Comments and labels

`//` starts a comment that runs to the end of the line. Comment lines directly
above a statement, with no blank line between, become that block's **note**
on the board; a comment at the end of a statement's line joins the note too,
and the formatter moves it above. Any other comment (above an `input`, after a
blank line, at the end of the file) produces a warning and is **dropped** when
the file is formatted, printed or published.

`@label("…")` gives a block the display name the board shows instead of its
handle.

```afs
scenario "Notes" {
  // Shown as the block's note on the board.
  // A second line of the same note.
  @label("Load the catalogue")
  products = GET "/products" {
    sample 200 { items: [] }
  }
}
```

A note goes above the statement's attributes, as here.

## What cannot be expressed

The text holds everything a run uses. It deliberately does not hold:

- **Layout**: where blocks stand on the board, whether they are pinned, and
  the board's extra copies of variables. The window keeps these beside the
  text and restores them by handle; a statement whose handle and kind did not
  change keeps its place.
- **Identities**: block and component ids never appear. A block is its handle;
  a component is its handle and revision.
- **Formatting**: there is one canonical layout, and comments survive only as
  notes.
- **Environment and secret values**: a file names them (`env.apiToken`); the
  project holds them.
- Loops, user-defined functions, imports, or more than one scenario per file.

The window may print attributes you do not need to write: `@note("…")` (a note
whose whitespace a comment cannot carry), `@config({ … })` and
`@inputs({ … })` (exact block settings and inputs with no shorter spelling).
Leave them as they are when you edit a file that has them. A `?` in a
printed file is a slot the board left unconnected; replace it with a value.

## A complete component

The component below is created and published as revision 1 of a test
project; the scenario after it calls that revision.

```afs
component find_product "Find a product" {
  description "Reads one product of the demo shop and checks that it is on sale."
  @label("Product ID")
  input productId: number
  @optional
  input currency: string = "EUR"
  action productId
  begin = start
  // The product as the shop sells it.
  @after(begin)
  response = GET "/products/{{param.productId}}" {
    headers { Accept: "application/json" }
    timeout 5s
    sample 200 { id: 123, name: "Whole bean coffee", price: 100, promotion: { type: "fixed_price", active: true, price: 80 } }
  }
  found = assert response.status == 200
  on_sale = assert response.body.promotion.active == true
  export name: string = "{{response.body.name}} ({{param.currency}})"
  export price: number = response.body.promotion.price
  export listPrice: number = response.body.price
}
```

## A complete scenario

```afs
scenario "Buy coffee" {
  description "Finds a product, puts it in a new cart and checks the total."
  on_error stop
  param productId = 123
  param quantity = 2
  begin = start
  // Product details from the shared component.
  @after(begin)
  product = find_product@1(productId: param.productId)
  @after(product)
  @label("Create a cart")
  cart = POST "/carts" {
    headers { Authorization: "Bearer {{env.apiToken}}" }
    body { productId: param.productId, quantity: param.quantity }
    sample 201 { id: "c-1", productId: 123, quantity: 2, total: 160, createdAt: 0 }
  }
  created = assert cart.status == 201
  var expected = product.price * param.quantity
  total_ok = assert cart.body.total == expected
  promoted = if product.price < product.listPrice {
    saving = product.listPrice * param.quantity - cart.body.total
    saved = assert saving > 0
  } else {
    full_price = assert cart.body.total == product.listPrice * param.quantity
  }
  cleanup {
    removed = DELETE "/carts/{{cart.body.id}}" {
      sample 200 { deleted: true }
    }
    gone = assert removed.body.deleted == true
  }
}
```

## Diagnostics and what to do

A diagnostic has a range in the file, a severity and a message; some carry a
fix. While a file has an error, the board keeps its last valid state and the
server's write tools do not save the text; a warning stops nothing. The
messages you will meet most:

- **``Expected `}` ``**, or the same with `)`, `]`, `:` or `=`: a brace or
  separator is missing; the fix inserts it. Count the braces of the block
  above the range.
- **`Unknown block handle`**: a name that is no statement of the file. Check
  the spelling, and that a main-flow statement does not use a cleanup handle
  (`Main flow cannot reference cleanup`).
- **`A block with multiple outputs needs an output name`**: a request, check
  or multi-export call is used whole. Write `.body`, `.status`, `.passed` or
  the export's name. **`Unknown block output`** names an output that does not
  exist.
- **`Unknown parameter`**: `param.x` with no `param x = …` in a scenario, or
  no `input x` in a component.
- **`Unknown environment name`**: not a variable or secret name of the
  project. Use the names the project lists; `env.baseUrl` always exists.
- **`Unknown request argument`**: `input.x` outside the request that declares
  `argument x = …`.
- **`Folder component calls require an explicit revision`**: add `@n`; the fix
  inserts the latest published revision.
- **`Component revision does not exist`**: publish the component first, or
  call a revision the project lists.
- **`Unknown operator or nested component call`**: a misspelt function, a
  component that is not published (then also `Operator calls cannot name a
  revision`), or a component call inside an expression (give it its own
  statement).
- **`Component arguments must be named`**, **`Unknown component input`**:
  call `handle@n(input: value)` with the component's input names.
- **`Duplicate block handle`**, **`Invalid or reserved block handle`**: rename
  the statement; reserved words and `env` and `param` cannot be handles.
- **`Parenthesize a comparison used as a check operand`**: write
  `assert (a == b) == true`, not `assert a == b contains c`.
- **`Expected a literal JSON value without interpolation`**: a parameter
  value, input default or sample refers to something or interpolates. Use
  plain JSON there.
- **`Export blocks belong in components`**, **`Only Value blocks may be marked
  var`**, **`Ordering must stay in its graph and branches need an IF`**: the
  form is in a place that does not allow it.
- **`Graph contains a data/control cycle`**: two statements need each other,
  through values or `@after`. Remove one direction.
- **`The value's type does not match this input`** (warning): a value of one
  type fills an input of another, such as text into a number input.
- **`This comment is not attached to a statement and will be dropped by
  formatting`** (warning): move the comment directly above a statement, or
  accept losing it.

When you work through the API Forge MCP server, its tools answer with codes as
well:

- **`stale_source`**: the file changed on disk since you read it; nothing was
  written. Read the source again (the answer carries the current hash), merge
  your change into it and write again with the new hash.
- **`comments_would_be_lost`**: publishing would drop the comments it lists,
  which belong to no statement. Move them above statements and publish again,
  or repeat the call with `accept_comment_loss: true` and the same hash if
  losing them is acceptable. The consent covers that text only.
- **`secret_in_text`**: the text contains the value of a known secret;
  nothing was written and the value is not repeated. Replace it with the
  secret's name, `env.name`.

`check_text` checks a proposal: it returns the diagnostics a text would have,
without saving it. `run` executes the saved source, not your proposal: write
the text first, then run it. A run is a simulation unless live requests are
enabled, and a simulation answers every request with its `sample`.
