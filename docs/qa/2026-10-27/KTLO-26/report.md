# KTLO-26: A board edit keeps the model the printed source reads back — QA report

- Branch: `fix/ktlo-26-rewire-source-order`; reproduction `8d115e6`
  (cherry-picked from `spike/ktlo-26-rewire-digest` `4ec99c9`), acceptance
  tests `be96cc8`, fix `4885f5c`.
- Mock-up: not applicable, no interface change. Ticket:
  [KTLO-26](../../../tickets/KTLO-26.md)
- Built with: `cargo test --locked` (debug). No release build and no captures:
  nothing on screen changes; what changes is the order of blocks and inputs in
  the session's model after a board edit, and with it the checkpoint the next
  save seals.
- Machine: Linux, shared throughout with other sessions building the
  workspace (load average 11–15), so timings are upper bounds.

## Scenarios

### 1. A board rewire seals a checkpoint that describes its source (criteria 1 and 2)

`ktlo_26_a_board_edit_that_rewires_a_check_seals_a_checkpoint_that_describes_its_source`
in `crates/apiforge-project/tests/acceptance.rs`: on the prepared demo
project, the scenario's cleanup loses the request whose status the check
compares, gains a new request (`delete_cart_again`) and has its status wired
back into the check's `actual`, through the same `delete_nodes`, `add_node`
and `connect` the Diagram mode uses; then `apply_model_edit`, `save`, and a
read-only reopen.

| Commit | Result |
| --- | --- |
| `8d115e6`, the spike's reproduction | ignored; run on the spike (analysis on #79, 10 of 10) it fails at `record.describes(…)` (sealed digest `0e5d5b72…`, the source's `41551dde…`) |
| `be96cc8`, acceptance before the fix | fails: the check's inputs in the session are `["expected", "actual"]`, expected `["actual", "expected"]`; the identities and positions assertion before it passes |
| `4885f5c`, after the fix | passes: block identities and positions and order-edge identities equal the edited model's; inputs `actual, expected`; the record describes the saved source; the reload gives the same block and input order and the saved model exactly |

### 2. The same rewiring typed as source (criterion 3)

`ktlo_26_the_same_rewiring_typed_as_source_seals_a_checkpoint_that_describes_it`
is unchanged from the spike and passes before and after the fix.

### 3. Both tests, looped

After the fix, the acceptance binary run alone with the filter `ktlo_26`, ten
times: **10 of 10** rounds `2 passed; 0 failed`.

### 4. The desktop sees no change (criterion 4)

No file under `crates/apiforge-desktop` changes. Its suite, whose board edits
go through `folder/publication.rs` into `apply_model_edit` — among them
`clicking_a_block_adds_it_at_the_visible_centre_selected_and_dirty` and
`dragging_a_block_onto_the_board_adds_it_at_the_drop_point`, which check the
new block's position and selection — passes unchanged: **420 passed, 0
failed, 24 ignored**.

### 5. No format or digest change; verification (criterion 5)

No file under `crates/apiforge-storage` changes. At `4885f5c`:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: `Finished`,
  no warning.
- `cargo test --workspace --all-targets --locked`: **1 093 passed, 0 failed,
  27 ignored** over 54 test binaries, exit status 0.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit
  status 0.

### 6. CI

The branch's CI run on `7d80824` (the fix plus this report's first version):
[40529343975](https://github.com/jakubiszon26/apiforge/actions/runs/40529343975),
**success** — `Build & Test` passed on `ubuntu-24.04` and on `windows-2022`,
and the Windows log shows both KTLO-26 tests `ok`. The later commit changes
only this section; its run is on the pull request.

## Windows

Not run locally. The change is platform-independent session code with no
interface; the only Windows evidence is the branch's CI run above, where the
whole suite passed on `windows-2022`, the two KTLO-26 tests among it.

## Not verified

- A board edit in the native window followed by a save and a reopen: covered
  through the session API in scenario 1 and by the desktop suite, not by hand.
- Checkpoints already sealed in the board's order: they still reconcile on
  load (identities and positions survive by handle) until their document is
  saved again; not rewritten by this ticket, by decision.
- Regression smoke list: not run. Nothing on screen changes.
- The budget and fuzzer commands of the development guide: not run; the
  change adds one derivation of the printed text per semantic board edit and
  touches neither the layout, the wire router nor the scenario language.

## Regression smoke list

Not run for this ticket; see *Not verified*.
