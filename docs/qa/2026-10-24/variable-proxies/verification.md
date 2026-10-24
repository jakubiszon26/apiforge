# Variable proxies (M3) — 2026-10-24

Verification of [M3](../../../roadmap.md#variable-proxies): the variable mark, the
places a value is shown (`graph.proxies`), nearest-proxy wiring and navigation
from a proxy to its variable.

## What was exercised

The board was rendered in the native window under Xvfb from a workspace built on
`fixtures/workspace.json`, with the Value `Produkty do dodania` marked as a
variable, a scenario parameter `ilosc` bound into it, and two proxies placed: one
for the variable beside the request that reads it, one for the parameter beside
the Value.

| Path | Outcome |
| --- | --- |
| `apiforge-desktop --check` on the document with proxies | Valid. The field round-trips through import and export. |
| `apiforge-desktop --screenshot` | Both chips are drawn, each wired to the consumer beside it. |
| `--select <proxy id>` | The proxy's variable and every block reading it are ringed, and its wires are picked out. |

- [Two values placed on the board](proxy-placed.png): the variable beside the
  request that reads it, the parameter beside the Value — the long wire across
  the board is gone, and the parameter, which has no block at all, is visible for
  the first time.
- [A selected proxy points at its variable and its uses](proxy-selected.png).

## The milestone's checks

| Requirement | How it is met |
| --- | --- |
| Proxies survive save/load | `graph.proxies` is part of the document; a storage session test saves and reloads a workspace with one, and the import test round-trips all three kinds of source and rejects a proxy that stands for a block that is not a variable of its graph. A graph without proxies writes no field, so earlier documents are byte-identical. |
| Proxies do not appear in code | The language has no syntax for layout. Printing a scenario before and after placing a variable gives the same text, and reconciliation puts the document's proxies back after a code edit, as it does positions. Two documents differing only in their proxies have the same semantic projection. |
| Deleting a variable removes its proxies in one undo step | `delete_nodes` drops them inside the same edit, and every applied edit prunes proxies whose variable is gone or no longer marked, so clearing the mark in the inspector or in code does the same. One undo brings the variable and its proxies back together. |
| A wire is never drawn twice for one consumer | Each reference yields exactly one wire; a proxy only changes where that wire starts. Tested with a variable at one end of the board and a consumer at the other, with no proxy, a near one, a far one and both. |

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: **555 passed, 0 failed, 1
  pre-existing ignored test**.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

## Limits

As in the earlier passes, the window was driven only through `--screenshot`:
`xdotool` is not available here, so dragging a chip out of the Variables panel,
moving one and dragging a wire out of one were exercised through the board's
commands and the editor, not through the real pointer. Mesa falls back to
software rendering under Xvfb, which affects nothing but speed.
