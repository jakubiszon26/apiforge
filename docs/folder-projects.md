# Folder projects

A folder project keeps every scenario and component as a `.afs` text file in an
ordinary folder, next to whatever else the folder holds. The files are the
definitions: they can be read, reviewed, diffed and merged with Git like any other
source. API Forge adds a small project file and a hidden `.apiforge` folder of its
own. This guide describes working with such a project in the desktop application.

Until folder projects become the default start (Stage II P7g), open one explicitly:

```bash
cargo run -p apiforge-desktop --locked -- --folder /path/to/project
```

The command-line tool reads the same folders; see the
[folder commands](development.md#folder-commands).

## What is in the folder

| Path | Contents | In Git |
| --- | --- | --- |
| `*.afs`, anywhere | Scenario and component sources registered in the project | Yes |
| `apiforge.project.json` | Name, registered documents, contracts and test environment. Secret values are never stored here, only their names | Yes |
| `.apiforge/diagram.json` | Block identities and positions, so a diagram keeps its layout | Yes |
| `.apiforge/local/` | This computer's lock, unsaved-work recovery, run journal and interrupted operations | No: `.gitignore` excludes it |
| `.gitattributes` | Keeps the files above at LF line endings in every checkout | Yes |

Creating a project, or migrating one, writes `.gitignore` and `.gitattributes`
entries without changing what was already there. Opening a project never rewrites
them. Other files in the folder, such as a `README.md`, can be opened and edited as
text but never become scenarios.

## Opening and switching projects

**Project → Open Folder…** opens the system folder chooser, starting next to the
current project. A folder that is not a project yet offers to initialise it and to
register the `.afs` files found in it; nothing is written until you confirm.
**Recent projects** in the same menu reopen earlier folders.

Started with `--managed`, API Forge opens the projects it keeps for you rather
than a folder you name. The first time, an existing workspace becomes folder
projects: the originals are left exactly as they were, the window says where the
projects now live, and an interrupted attempt continues rather than making a
second copy. With nothing to migrate you get one project to start from. This is
not yet how API Forge starts by default.

Switching first protects the current project's unsaved work. If that protection
fails, you stay in the current project. Only one window can change a project at a
time; a second window opens it read-only, says so, and can still export its work.

## The window

- **Explorer** (left): the project folder as a tree. One click opens a file.
  Right-click, or the buttons above the tree, create, rename, move and delete files
  and folders. Deleted files go to the project's **Trash**, which restores them.
- **Tabs**: one per open file. A dot marks unsaved work; the close button asks
  before discarding it.
- **Code, Diagram and Split** (top): the same document as text, as a board of
  blocks, or both side by side. Edits in either form update the other once the
  code is valid; invalid code keeps the last valid diagram read-only.
- **Problems** (bottom): errors and warnings of the active source, with their line
  and column.
- **Inspector** (Diagram and Split): the selected block's fields and arguments.

## Saving and unsaved work

Ctrl+S saves the active file exactly as typed, including invalid code; Ctrl+Shift+S
saves everything, including project settings. Until you save, unsaved work is kept
in `.apiforge/local/` and returns when the project is reopened, even after a crash.
If the file changed on disk in the meantime, the restored work waits for you to keep
or discard it.

A file changed or deleted outside API Forge reloads by itself when it has no unsaved
work. If it does, saving pauses and **Review conflict…** shows both versions; you
choose which to keep, and either can be exported first. An operation interrupted by
a crash opens a recovery review before anything else, with the same choice.

## Project settings and the test environment

**Project → Project settings…** edits the project name and its data contracts.
**Project → Test environment…** sets the base URL, plain variables and session
secrets. Apply marks the project file as unsaved; Save all writes it. Secret values
stay in memory for the session and are never written or exported.

## Runs

**Run** simulates the active scenario with its sample responses. **Run options →
Run live requests** sends real requests. A run takes the scenario's current text,
saved or not, together with the published component revisions it calls; later
edits do not change a run in progress. When the code has errors, Run offers the
last valid version and asks first. A live run is journalled under
`.apiforge/local/`; one interrupted by a crash is listed at the next start until
marked as handled. The command-line `run --folder` uses saved files only.

## Large sources

A source can be up to 1 MiB. Above 256 KiB the editor shows it without colouring,
completion opens only on Ctrl+Space, and the diagram follows the caret once the
latest edit has been checked. The text itself is unaffected, and saving keeps it
exact.

## Keyboard

| Keys | Action |
| --- | --- |
| Tab, Shift+Tab | Move between controls in reading order |
| Space, Enter | Activate the focused button, open the focused menu |
| Escape | Close a menu or cancel a dialog |
| Ctrl+S, Ctrl+Shift+S | Save, save all |
| Ctrl+E | Switch between Code and Diagram |
| Ctrl+Tab, Ctrl+Shift+Tab | Next, previous tab |
| Ctrl+W | Close the active tab |
| Ctrl+Z, Ctrl+Y (Ctrl+Shift+Z) | Undo, redo: the text in a text field, otherwise the diagram |
| Ctrl+Space | Offer completions in code |
| Arrow keys, Enter, F2, Delete | Explorer: move and fold, open, rename, delete |
| Ctrl+D, Ctrl+C, Ctrl+V | Diagram: duplicate, copy, paste blocks |

In the code editor Tab indents. Press Escape first, then Tab, to move on to the
next control. Every control carries a name for screen readers, and the explorer is
exposed as a tree.

## Git

Commit the `.afs` files, `apiforge.project.json`, `.apiforge/diagram.json` and
`.gitattributes`. Two branches that change different scenarios merge cleanly, and
the merged project opens with both changes. Concurrent changes to the same scenario
can still conflict like any source file.

Published component revisions are checked byte for byte. Keep the generated LF
rules, and review any nested `.gitattributes` that would convert `.afs` files to
CRLF: a converted revision is reported as damaged rather than silently accepted.
Close API Forge, or let it reload, before a checkout that changes many files at
once; it notices the change but cannot make another program's checkout atomic.
