# Working on the Rust version

The native application lives in the Cargo workspace under `crates/`. Its scope and the
order of work are described in the [migration plan](../../RUST_MIGRATION.md). The
TypeScript prototype is still started with `npm run dev`.

## Requirements and running

Rust 1.99.0 with rustfmt and clippy; the version is set by `rust-toolchain.toml` and
dependencies by `Cargo.lock`.

On Linux a working X11 or Wayland session is needed, a driver compatible with the WGPU
backend, the system libraries for building and a file portal for the Open dialog. The
Ubuntu package list is in the [CI workflow](../../.github/workflows/rust.yml). Minimum
distribution and Windows versions and packaging still need acceptance.

```bash
cargo run -p apiforge-desktop --locked -- --project fixtures/workspace.json
```

Without a graphical environment a file can be checked:

```bash
cargo run -p apiforge-desktop --locked -- --check fixtures/workspace.json
```

Check mode returns 0 for a valid import, 1 for a document/read error and 2 for invalid
arguments. Field and secret values are never printed in parser diagnostics. Import is
limited to 16 MiB and performs no HTTP requests.

## Demo API

The built-in demo projects (`apiforge_core::demos`) send their requests to a small local
API on `http://127.0.0.1:4318/demo`. Start it in a separate terminal:

```bash
cargo run -p apiforge-demo-api --locked
cargo run -p apiforge-demo-api --locked -- --port 4400
```

It serves `GET /demo/shop/products/{id}` (products 123 and 124), `POST /demo/shop/carts`,
`POST /demo/shop/carts/{id}/items`, `GET`/`DELETE /demo/shop/carts/{id}` and
`GET /demo/support/tickets/{id}` (ticket 7). State is kept in memory only: at most 1000
carts, each expiring after an hour and holding at most 100 item lines. The server only
listens on loopback and rejects requests for other hosts or from foreign web origins.
Changing the port means editing the base URL in the demo project's environment.

Tests and the desktop application can run it in-process with
`apiforge_demo_api::spawn(addr)`; dropping the returned handle stops the server.

## Finding steps and saving safely

Use **Find step…** or **Ctrl+K** to search the current board by name or block type.
Arrow keys choose a result, Enter opens its configuration and brings it into view,
and Escape dismisses the search. Text-field paste remains separate from block paste.

Autosave keeps only the latest waiting workspace while an earlier write is in
progress. The saved status acknowledges that generation only. Closing flushes pending
edits and waits for the result; a failed save keeps the window open with **Retry save**.
After fixing the underlying storage problem, retry saves the current document.

If the saved workspace cannot be loaded, the application opens a temporary session
with autosave disabled and offers normal JSON export. Existing files remain untouched.
A workspace from a newer format version is never replaced with an older backup.
Truncated documents can still recover from a validated backup.

The native application uses format version 2. Importing version 1 reports a converter
hint; it never falls back to a different workspace or overwrites the source. At startup,
an existing version-one workspace and its backup are preserved byte for byte as
`workspace.v1.json` and `workspace.v1.json.bak` before a fresh demo workspace is opened.
If either archive name is already taken or archiving fails, autosave stays disabled
and the originals are preserved. Filesystems without hard-link support take this safe
temporary-session path. Multiple writers and external edits remain S1 work.

To convert an old document during development, write to a new output path and import it:

```bash
cargo run -p apiforge-convert-v1 --locked -- workspace.v1.json workspace-v2.json
cargo run -p apiforge-desktop --locked -- --project workspace-v2.json
```

On Unix, successful writes include file and directory sync, including newly created
parent directories; sync failures are surfaced. Windows uses flushed files and atomic
replacement, but power-loss durability of directory metadata remains an explicit
platform limitation pending native release verification.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --release -p apiforge-desktop --locked
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

The egui tests lay out every project and both phases without a GPU; they are not tests of
the whole native backend.

A real window start-up test opens a document, draws at least three frames and closes the
application. It needs a graphical session:

```bash
cargo run -p apiforge-desktop --locked -- --smoke-test fixtures/workspace.json
```

A screenshot of a document can be captured for visual review:

```bash
cargo run -p apiforge-desktop --locked -- --screenshot out.png --project fixtures/workspace.json
```

## Compatibility corpus

Only the expression semantics corpus is generated from the TypeScript sources:

```bash
node --import tsx scripts/export-compatibility-fixtures.ts
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Never format the generated JSON by hand: its bytes and SHA-256 must match the generator.
See [fixtures/compatibility/README.md](../../fixtures/compatibility/README.md).
The native v2 project and run fixtures are generated from frozen v1 sources by
`cargo run -p apiforge-convert-v1 --locked -- --fixtures`; add `--check` to verify them.
See [fixtures/README.md](../../fixtures/README.md).

## Version control

Work happens on short-lived `feat/…`, `fix/…` or `docs/…` branches with small
[Conventional Commits](https://www.conventionalcommits.org/), merged into `main` after the
verification commands above pass.
