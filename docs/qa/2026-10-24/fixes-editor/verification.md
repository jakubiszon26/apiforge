# Code editor fixes — 2026-10-24

Third pass, after [BUG-001–017](../fixes/verification.md) and
[BUG-018–025](../fixes-next/verification.md). It began with a report that
Backspace does not work in the code editor; the cause turned out to be shared
with the rest of the editor's key handling, so Tab was examined the same way,
and the drawing and printing the panel repeats every frame with it.

| ID | Failure | Fixed behaviour / verification |
| --- | --- | --- |
| BUG-026 | `auto_indent` ran for any change that left the caret after a newline, so Backspace over the indentation saw it written straight back and a line break could not be deleted. | Indentation follows a line break and nothing else. The editor asks the frame's own input whether the line is being broken, after the completion popup has taken its keys, so an Enter that accepts a completion is not mistaken for one. Regressions walk Backspace off the indentation and off the line break, join two lines at a line start, and check that a paste ending in a newline gains no indentation. |
| BUG-027 | Tab over a selection deleted it and left a tab character. | Tab indents every line a selection touches and keeps the selection, so Shift+Tab puts it back. A regression selects two statements, indents, dedents and compares the text both ways. |
| BUG-028 | Tab inserted `\t`; Shift+Tab removed egui's four spaces, missing a two-space line entirely. | The editor takes Tab before the text edit sees it. A caret of its own moves to the next two-space stop; Shift+Tab takes exactly one level off the line. Regressions cover both widths and the unit behaviour of `reindent`, including a reversed selection and a selection ending at a line start. |
| BUG-029 | The gutter asked the galley for every line's position from the first one, which is linear in the text and in the rows, once per line, per frame. | The gutter walks the galley's own rows and counts line breaks, keeping a wrapped line's number beside its first visual row. A 30 000-line buffer scrolled to the end went from **921 ms to 0.71 ms per frame** (release, same harness, measured before and after). A regression reads the painted shapes back and checks that every line is numbered once and only a flagged line is marked. |
| BUG-030 | Auto-indent deepened after `{` only. | `[` and `(` open a block too, which is how `print_json_body` and the formatter write them. Covered in the auto-indent test with a JSON body. |
| BUG-031 | `code_panel` printed the document every frame, cloning and validating the whole project first: 4.5 ms at the two hundred blocks a graph may hold. | The printed text is kept until the document changes. The key is the buffer context, `Editor::revision` and a new `Editor::context_generation`, which counts moves between documents — a component draft left unsaved and opened again is the saved definition once more, and no revision records that. A regression discards such a draft and re-enters it with no frame in between, and fails if the panel serves the discarded text. |

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: **536 passed, 0 failed, 1 pre-existing ignored test** (525 before this pass).
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

Added along the way: a property test that calls everything the editor runs on
every keystroke — the lexer, the parser, lowering, completion, hover and
statement lookup — over a thousand generated half-typed sources at every caret
position. It found no panic, and guards the editor's hot path against one.

## Limits

Every finding here was reproduced and verified in the editor's own egui harness
and the application harness, driving real key events through the same widgets
the window uses. **This pass includes no native X11 retest and no screenshots**:
the tools the earlier passes used for that (`xdotool`, `xclip`) are not present
in this environment, so the native window could not be driven. The timings are
release-build measurements from the harness, taken with the same code path the
panel draws, not from the running application.
