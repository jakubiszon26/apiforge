# End-to-end pass — 2026-10-24

Fourth pass, after [BUG-001–017](../fixes/verification.md),
[BUG-018–025](../fixes-next/verification.md) and
[BUG-026–031](../fixes-editor/verification.md). This one ran the whole
application rather than its parts: the demo API on loopback, the command line
against it, and the native window under Xvfb with Mesa Lavapipe.

## What was exercised

| Path | Outcome |
| --- | --- |
| `apiforge-demo-api` on 127.0.0.1:4318 | Serves the shop and support demo API. |
| All four demo scenarios, `apiforge run --live --json` | **Passed**, every step, against the real API. |
| `apiforge print` → edit → `apiforge apply` → `run` | The code pipeline round-trips through the command line; a broken assertion applied this way then fails the run with exit code 1. |
| A scenario pointed at a closed port | Reports the transport failure and blocks the steps that depended on it. Surfaced BUG-033. |
| A failed comparison | Exit code 1, `status: failed`, `expected`/`actual` in the report. Surfaced BUG-034. |
| `apiforge-desktop --smoke-test` | Opens, draws and exits cleanly. |
| `apiforge-desktop --screenshot` for all four demo projects | All four boards render. Surfaced BUG-032. |

## Fixes

| ID | Failure | Fixed behaviour / verification |
| --- | --- | --- |
| BUG-032 | The two cards a wire joins were left out of the router's obstacles, so a route could cross them. An ordering wire between two cards side by side went from the `Next` port straight up through its own card, which is painted over it. | A wire's own cards are obstacles like any other, with the same clearance. The stub is still measured without them, so a port on a card's edge keeps somewhere to start, and the ladder of widening attempts ends with the old behaviour for cards that leave no way round at all. The demo route became `(180,176) → (180,188) → (358,188) → (358,84) → (510,84) → (510,100)`: down out of the port, round, and into the target. A regression checks that the first segment leaves the port perpendicular and that no segment crosses either card. |
| BUG-033 | `print_report` walked only the top-level steps. | A component's inner steps are printed indented beneath it. The regression runs live against a closed port and requires an indented line naming the failed request. |
| BUG-034 | The printer ignored `expected` and `actual`. | A step that did not pass prints both, each shortened to 120 characters so a large body cannot bury the verdict. The regression requires both lines, and requires that a step which passed prints neither. |

Before and after, from the native window:

- [The wire appears above `begin`, joined to nothing](BUG-032-wire-under-its-block.png)
- [The wire leaves the `Next` port and goes round](BUG-032-wire-leaves-its-port.png)

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: **539 passed, 0 failed, 1 pre-existing ignored test**.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
- All four demo boards re-rendered after the routing change; the three without
  ordering wires are unchanged.

## Limits

The native window was driven only through `--smoke-test` and `--screenshot`:
`xdotool` and `xclip` are not available here, so no pointer or keyboard
interaction was replayed against the real window. Everything else was driven
through the shipped binaries. The live runs reached only the local demo API; no
external service was called.
