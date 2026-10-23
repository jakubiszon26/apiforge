# API Forge

A desktop tool for visual REST API regression tests. A team defines its own requests,
components, data contracts and business actions on a graph; the engine assumes no domain
and no endpoint names.

The repository contains two implementations:

- **Native application (Rust + egui)** in `crates/` — the production version, being
  ported to feature parity. Linux and Windows are first-release targets. See the
  [migration plan](RUST_MIGRATION.md) and the
  [developer guide](docs/migration/development.md).
- **TypeScript prototype (React + Express)** in `app/`, `api/` and `shared/` — the
  behavioural reference for the port. It stays until the native application replaces it.

## Native application

```bash
cargo run -p apiforge-desktop --locked
```

Verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Design decisions are recorded in [docs/decisions](docs/decisions).

The scenario language and workspace-aware CLI are available separately:

```bash
cargo run -p apiforge-cli --locked -- fmt fixtures/scripts/checkout.afs --check
cargo run -p apiforge-cli --locked -- fmt path/to/scenario.afs
cargo run -p apiforge-cli --locked -- check --workspace fixtures/workspace.json
cargo run -p apiforge-cli --locked -- print --workspace workspace.json --scenario <id>
cargo run -p apiforge-cli --locked -- apply --workspace workspace.json --scenario <id> scenario.afs
```

`fmt` checks syntax and formats in place. `check` resolves source against the active
project, `print` projects a stored scenario to code, and `apply` reconciles a
scenario with stable identities and saves through an exclusive storage session.
These commands do not execute requests. M4 remains in progress; see
[the language implementation notes](docs/migration/scenario-language.md).

## TypeScript prototype

```bash
npm install
npm run dev
```

Open **http://127.0.0.1:5173**. The frontend runs on port 5173 and the local runner on
4318; both listen locally only. The prototype UI is in Polish.

Built variant:

```bash
npm run build
npm start
```

The backend then serves the built interface at **http://127.0.0.1:4318**.

Verification: `npm run typecheck`, `npm test`, `npm run test:e2e`, `npm run build`.
Browser tests use the system `/usr/bin/chromium` when available; otherwise install
Chromium for Playwright (`npx playwright install chromium`) or point
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` at a browser. Formatting: `npm run format` and
`npm run format:check`.

## How it works

### A first scenario

1. Open the shop or ticketing demo project, or create an empty project.
2. A new scenario contains a manual **Start**. Add a block from the library by clicking or
   dragging. Configure it in the right panel; chosen arguments can be edited directly on
   the block.
3. Connect **Next** to the top input of the following block, or choose the next step in
   the configuration. The first request of a new scenario is connected to Start
   automatically. Right-click a block or choose **Action**: the project actions offered
   follow from the output type, and the chosen action creates a connected step.
4. Bind data sources to named inputs, or choose an earlier step and a JSON path in the
   argument configuration.
5. In the output data, click a response field to add a check for it.
6. Choose **Simulation** or **API** and run the test. Simulation uses explicitly saved
   samples; a live run sends real requests.
7. Open a step result: the report shows inputs, outputs, request and response, expected
   and actual values and the inside of custom components.

A full run of the shop project in live mode uses the local demo API: it creates a cart,
checks the price and deletes the cart in the **After test** section. Product `123` has a
promotional price of 80 and `124` of 90; changing the ID while expecting 80 shows a
detected discrepancy.

### Ordering and IF / ELSE

**Start** runs the connected path. Each request finishes before the next one starts.
**IF / ELSE** takes the same arguments and operations as Comparison, but its result
selects the **Yes** or **No** output; false by itself is not a test failure.

Steps outside the path from Start and unchosen branches are skipped without requests.
Data adds dependencies: a block waits for the sources of all its arguments. Alternative
ordering connections can merge into a shared step; one active branch is enough, but the
arguments must still be available. A reference to the response of a skipped branch never
runs it. Older graphs without Start keep the previous behaviour of running every root.

The workflow example fetches a product price, checks for status 200 in IF, creates an
empty cart and only then adds products; the final Comparison checks the total and the
**After test** section deletes the cart. Changing the product endpoint to `/products/999`
shows IF → No and no cart being created.

### Custom components

Select steps on the graph (Ctrl + click) and save the selection as a component. Choose
public arguments, returned data and an optional argument fed from the **Add action**
menu. Components can contain other components; data contracts are defined in the project
settings.

Editing the inside of a component creates a new revision. Existing instances keep their
revision; update a chosen instance in its configuration. The original fragment stays on
the graph after the component is saved.

### Operators, variables and response fields

Any argument can hold a nested **operator** block. 37 operators in six groups cover
mathematics, comparison, logic, text, functions and JSON data. Mathematics and logic
require proper types: missing data and null are not zero. See
[docs/poc/operator-blocks.md](docs/poc/operator-blocks.md).

**Variables** are separate blocks with a name and a value of any JSON type. Dropping a
variable on an argument creates a type-preserving binding; dropping it into a URL, query
value, header or select path inserts `{{name}}` at the caret and binds the source. A
variable can take a response field: the assignment stores `request → output → path`, so
every run reads a fresh value. `null`, `false` and zero remain valid values; a missing
field is an error.

**Check** next to a field in the output data or the report creates a **Comparison** with
the actual and the expected value. Its operation can be equality, containment, "has type"
and more; both arguments can come from a literal, another step, a parameter or the
environment.

On the board, **Ctrl+C** copies the selection with its configuration, **Ctrl+V** pastes
an independent copy and **Ctrl+D** duplicates. Internal connections of the group are
remapped to the copy; references to other blocks keep their sources.

### Data and execution

- Projects are saved locally; JSON export/import keeps a copy and moves definitions.
- Environment secrets are kept in memory for the session only and never reach a project
  export. Authorisation headers and recognised secret values are masked in reports. Do
  not type tokens as ordinary literal values in a project.
- The base URL may contain a prefix, e.g. `https://example.test/api/v1`. The path
  `/users/:id` is appended to it; a full URL bypasses the base URL.
- Add named request arguments in **Arguments**, choose their value sources and use
  `{{input.productId}}` in the URL, query or headers. For a typed body value, choose
  **Request argument** as its binding source, or enter
  `{"productId": "{{input.productId}}"}` directly in the **Body** editor. Whole
  placeholders keep their JSON type; text around them produces a string. Text
  bodies accept templates too. Argument fields can be objects or arrays;
  `input.product.id` selects a field. URL values are percent-encoded.
- HTTP 4xx/5xx is data to check. Add a status assertion to accept only chosen responses.
- After an error the scenario can stop or continue with the steps that can still run. An
  unmet data condition blocks dependent steps. Cleanup runs separately and also after an
  error or a controlled cancellation.
- Running up to a chosen block is a partial run. A single step may use explicitly chosen
  data of an earlier run; its result is diagnostic.

### Limitations

- Single user, local application; no real-time collaboration and no public runner.
- Acyclic graphs executed sequentially; no loops, case matrices or JavaScript scripts.
- HTTP requests with JSON or text responses; no multipart, automatic OAuth, automatic
  retries or HTTP redirects.
- Request timeout up to 60 seconds, 2 MiB response limit, 120-second scenario budget and a
  separate 10 seconds for cleanup.
- Cancellation does not undo operations performed by the tested API. A process crash
  cannot guarantee cleanup.

Historical prototype documents: [docs/poc](docs/poc). The earlier mock-up is in
`design-preview/`.
