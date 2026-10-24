# API Forge

API Forge is a desktop application for visual REST API regression tests. A team
models requests, checks, reusable components, data contracts and business actions as
a graph of blocks. The engine assumes no domain and no endpoint names.

Every scenario is also text: the scenario language prints a board as code and applies
edited code back to the board without losing block identity or layout.

It is written in Rust with [egui](https://github.com/emilk/egui). Linux and Windows are
the first release targets.

## Features

- **Graph editor.** Requests, Values (literals, operators, objects, arrays,
  templates and variables), assertions, data conditions, IF/ELSE branches, component
  instances and Export blocks, wired by data and by explicit ordering. Blocks are
  arranged by automatic layout, and wires go around the blocks rather than through
  them, with right angles or curves.
- **Components.** A selection becomes a reusable, revisioned component with typed
  inputs, Export outputs and an optional business action.
- **Runs.** Simulation mode uses saved sample responses; live mode sends real requests.
  Reports show every step's inputs, outputs, request and response, with secrets masked,
  and say whether a failed request can have changed anything. An independent cleanup
  phase also runs after errors and cancellation. A live run is journalled, so a run
  interrupted by a crash is reported at the next start.
- **Scenario language.** A lossless parser, a formatter and a CLI to check, print and
  apply scenarios against a workspace. Request bodies and the URL, header and query
  fields are edited as code, with completion for blocks, variables, parameters,
  environment names, components, operators and response paths.
- **Safe local storage.** Atomic saves with a validated backup, a single writer per
  workspace, detection of external changes and deliberate recovery.

## Getting started

Requirements: Rust 1.99.0 (pinned in `rust-toolchain.toml`). On Linux an X11 or
Wayland session and the system libraries listed in the
[CI workflow](.github/workflows/rust.yml) are needed.

```bash
cargo run -p apiforge-desktop --locked
```

The first start opens demo projects. They run against a small local API:

```bash
cargo run -p apiforge-demo-api --locked   # http://127.0.0.1:4318/demo
```

## Scenario language

```text
scenario "Checkout" {
  param quantity = 2
  begin = start
  @after(begin)
  products = GET "/products" {
    sample 200 { items: [{ id: "p-1", price: 12.5 }] }
  }
  status_ok = assert products.status == 200
  first = products.body.items[0]
  @after(status_ok)
  order = POST "/orders" {
    body { productId: first.id, quantity: param.quantity }
  }
}
```

```bash
apiforge fmt scenario.afs [--check]
apiforge check --workspace workspace.json [--scenario <name|id>] [scenario.afs]
apiforge print --workspace workspace.json --scenario <name|id>
apiforge apply --workspace workspace.json --scenario <name|id> scenario.afs
apiforge run --workspace workspace.json --scenario <name|id> [--live] [--secrets secrets.json] [--json]
```

Run the CLI with `cargo run -p apiforge-cli --locked -- <command>`. `apply` keeps the
identity and position of unchanged blocks and saves through the same exclusive
session as the desktop application. `run` executes the saved scenario: a simulation
by default, real requests with `--live`, which takes the writer lock so that what it
sends is journalled. Secrets come from `--secrets` or from `APIFORGE_SECRET_<name>`
in the environment, never from the command line. The grammar and the round-trip contract are
specified in [ADR 0005](docs/decisions/0005-scenario-language.md).

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/apiforge-core` | Document model, validation, bindings, operators, graph semantics and editing operations |
| `crates/apiforge-runner` | Execution engine, HTTP transport, redaction and the background run service |
| `crates/apiforge-storage` | Workspace files, atomic saves, backups and single-writer sessions |
| `crates/apiforge-script` | Scenario language: lexer, parser, formatter, lowering, printer and reconciliation |
| `crates/apiforge-layout` | Layered automatic layout and orthogonal wire routing, as pure geometry |
| `crates/apiforge-cli` | The `apiforge` command-line tool |
| `crates/apiforge-desktop` | The egui application |
| `crates/apiforge-demo-api` | Local HTTP API used by the demo projects |
| `tools/convert-v1` | Development tool converting version-one documents and regenerating fixtures |
| `fixtures` | Projects, recorded runs, the semantic corpus and example scripts |
| `docs` | [Development guide](docs/development.md), [roadmap](docs/roadmap.md) and [architecture decisions](docs/decisions) |

## Limitations

- Single user and local only; no collaboration and no hosted runner.
- Acyclic graphs executed sequentially; no loops, case matrices or scripting.
- JSON and text bodies; no multipart, automatic OAuth, retries or redirects.
- Requests time out after 10 seconds unless configured (at most 60), responses are
  limited to 2 MiB, the main phase to 120 seconds and cleanup to 10 seconds.
- Cancellation cannot undo operations already performed by the tested API, and a
  process crash cannot guarantee cleanup. The run journal reports what such a run
  may have performed; it does not compensate for it.
