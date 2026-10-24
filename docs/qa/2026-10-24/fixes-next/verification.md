# Additional desktop fixes — 2026-10-24

Follow-up to [BUG-001–017](../fixes/verification.md). These findings came from
additional regression scenarios; all fixes are in the working tree.

| ID | Failure | Fixed behavior / verification |
| --- | --- | --- |
| BUG-018 | Switching the board phase, including moving the code caret into cleanup, clears document Undo/Redo. | Both phases share a chronological scenario history. Snapshots record their target phase; undo/redo affect the right graph after navigation. Tests cover whole-code edits and interleaved graph edits. Native Undo after caret navigation restores main 1 and teardown 2 together. |
| BUG-019 | Code widgets share one egui identity across documents; Ctrl+Z in another scenario restores the previous scenario's text. | Text-edit and scroll identities belong to the scenario/component context. Transient completion/reveal state resets on context changes. An egui regression and native two-scenario test leave the second scenario unchanged by Ctrl+Z. |
| BUG-020 | Closing Code before debounce leaves valid text unapplied and the board locked; Ctrl+E also does not work while typing. | The buffer schedules its debounce and applies independently of panel visibility. Ctrl+E is handled before text focus suppresses document shortcuts. A regression and native paste-then-Ctrl+E test apply `answer = 42` with Code closed. |
| BUG-021 | A failed draft write reports a notice but allows the window to close, losing the only copy of unfinished text. | Draft errors participate in close readiness, show “Drafts not saved” and Retry save, and clear after successful persistence. The regression blocks the draft filename with a directory, verifies close is refused, removes the obstruction and verifies retry saves the text. Close also settles active pending code before saving. |
| BUG-022 | Moving the caret forward or selecting text leaves completion open for its old replacement range. | Navigation/selection dismiss the obsolete popup. A regression moves beyond the prefix and confirms Enter cannot insert the old offer. |
| BUG-023 | Restored draft cursors are stored but ignored by the text widget. | The stored caret is installed before layout/input, using character indices derived from UTF-8 byte offsets. A regression restores a position after Polish characters and types at the correct location. |
| BUG-024 | Input immediately after accepting a completion is handled before the completion's new caret is installed. | Code and template editors prepare programmatic caret changes before consuming the next text event. The regression accepts `products`, immediately types `.`, and obtains `products.`. |
| BUG-025 | After a board edit reprints code, text Undo can resurrect code from before that board edit. | A document reprint clears obsolete text undo/completion state while retaining context-local widget state. The regression edits text, changes value 1 to 2 on the board, focuses Code and verifies Ctrl+Z retains the current printed document. |

## Verification

- `cargo test --workspace --all-targets --locked`: **525 passed, 0 failed, 1 pre-existing ignored test**.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: passed.
- Native debug window under Xvfb/X11 with Mesa Lavapipe, isolated data and preferences:
  pending code with Ctrl+E, Undo after phase navigation, a new scenario's text Undo,
  and refusal to close after an injected draft-write failure. Removing the directory
  obstruction lets the draft save; readback retained the marker and the window closed.

Native evidence:

- [Hidden code applied after Ctrl+E](hidden-code-applied.png)
- [Both phases restored after caret navigation](caret-phase-undo.png)
- [Second scenario retained after text Undo](second-scenario-after-undo.png)
- [Close blocked while the draft cannot be saved](close-blocked.png)

The automated tests exercise real egui text events and the application's persistence
and close paths. Native testing is a selected Linux retest, not full Windows or
Wayland coverage. No API requests were sent by the native probes; HTTP tests use
local loopback listeners.
