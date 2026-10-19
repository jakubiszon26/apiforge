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
cargo run -p apiforge-desktop --locked -- --project fixtures/compatibility/workspace-v1.json
```

Without a graphical environment a file can be checked:

```bash
cargo run -p apiforge-desktop --locked -- --check fixtures/compatibility/workspace-v1.json
```

Check mode returns 0 for a valid import, 1 for a document/read error and 2 for invalid
arguments. Field and secret values are never printed in parser diagnostics. Import is
limited to 16 MiB and performs no HTTP requests.

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
cargo run -p apiforge-desktop --locked -- --smoke-test fixtures/compatibility/workspace-v1.json
```

A screenshot of a document can be captured for visual review:

```bash
cargo run -p apiforge-desktop --locked -- --screenshot out.png --project fixtures/compatibility/workspace-v1.json
```

## Compatibility corpus

The comparison corpus is generated from the TypeScript sources:

```bash
node --import tsx scripts/export-compatibility-fixtures.ts
node --import tsx scripts/export-compatibility-fixtures.ts --check
```

Never format the generated JSON by hand: its bytes and SHA-256 must match the generator.
See [fixtures/compatibility/README.md](../../fixtures/compatibility/README.md).

## Version control

Work happens on short-lived `feat/…`, `fix/…` or `docs/…` branches with small
[Conventional Commits](https://www.conventionalcommits.org/), merged into `main` after the
verification commands above pass.
