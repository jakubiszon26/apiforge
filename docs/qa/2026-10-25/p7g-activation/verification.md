# Stage II P7g: managed startup, migration and project creation

Branch: `feat/stage-ii-p7-activation`. Base: unmerged P7 integration `09387b0`
(`feat/stage-ii-p7-integration`), which merged the P7 interface tests into P7f.
Implementation: `a947639`, `73d356a`.

Everything below was run against the release binary on Linux (Wayland, X11
fallback), each run in its own `XDG_DATA_HOME` so none of it touched the data
directory this computer actually uses.

## An empty data directory

`apiforge-desktop --managed` against a data directory with nothing in it:

```
<data>/apiforge/projects/shop/apiforge.project.json
<data>/apiforge/projects/shop/.gitattributes
<data>/apiforge/projects/shop/.gitignore
<data>/apiforge/projects/shop/components
<data>/apiforge/projects/shop/scenarios
<data>/apiforge/recent-projects.json
```

No `workspace.json` and no `projects.json` were written: nothing was migrated,
so no migration is claimed. `recent-projects.json` records the new project as
`active`, which is what reopens it on the next start. The project holds the
demo, so the window opens on a scenario rather than an empty board
([created-project.png](created-project.png)).

Starting again opened the same project; the `projects` directory still held one
entry, so a created project is picked up rather than duplicated.

## A legacy workspace

A version-two workspace of the four demo projects was placed at
`<data>/apiforge/workspace.json` and `--managed` was started once.

| Check | Result |
| --- | --- |
| Projects migrated | `shop`, `tickets`, `blocks-poc`, `workflow-poc`, each at `projects/<project-id>/` |
| Catalogue activated | `projects.json` with migration ID `050cb2fb…`, and the workspace hash as its input |
| Original workspace | `sha256 4e68b6fb…` before and after — byte-identical |
| Opened project | the one the workspace had active |
| Report | the window named the new projects directory and the untouched original |

The migrated project opens, resolves its component revisions and validates
("No problems", "Saved"), and its explorer shows both Git rules
([migrated-project.png](migrated-project.png)).

Restarting `--managed` kept migration ID `050cb2fb…` and the same four
projects: a completed migration is read, not repeated.

## What is left to the workspace window

Two conditions are answered before a window exists, because only the workspace
window can resolve them and which window to build cannot wait:

| Condition | Behaviour |
| --- | --- |
| Another window owns the workspace | The workspace window opens, exactly as a second window does today. Nothing is migrated while another window holds it |
| The workspace is waiting on a backup decision | The workspace window opens with its own recovery prompt, and the reason is printed. No catalogue is written |

Both are covered by interaction tests rather than only by a native run, because
each needs a workspace in a state that is awkward to produce by hand.

## Failure

An occupied destination stops migration before activation: the foreign bytes
were preserved, no `projects.json` was written, and the error named the project.
Removing the obstruction let the same migration finish. There is no rollback,
because there is nothing to roll back to — the originals were never changed.

## Verification

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p apiforge-desktop --all-targets --locked --features inspection -- -D warnings
cargo test --workspace --all-targets --locked
cargo run -p apiforge-convert-v1 --locked -- --fixtures --check
cargo test -p apiforge-desktop --release --locked frame_budgets -- --ignored
tools/qa/ui-quality/run.sh HEAD
```

All passed: 949 workspace tests, eighteen ignored (two helpers, fourteen review
views, the inspection host, the release frame test); UI gate 10/10; release
frame budgets held at 1000 lines and 1 MiB.

## Limits

- **Windows is not closed by any of this.** Native Windows QA and Windows
  directory durability remain open release gates; a Linux run cannot stand in
  for either, and migration writes and renames directories, which is exactly
  where the two differ.
- **The default start is unchanged.** `apiforge-desktop` with no arguments still
  opens the legacy workspace. Managed startup is reachable only through
  `--managed` until those gates close; flipping it is a change to one match arm.
- No screen-reader session was run; the accessibility evidence is the named
  widgets and the tree the interaction tests assert on.
