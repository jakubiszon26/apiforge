# Migrating the interface to Rust + egui

Audit state: 19 October 2026. This document describes migration requirements; it does
not claim they are implemented. It is based on the current application files and tests;
a new window skeleton alone does not satisfy them. By the user's decision, Linux and
Windows are first-release targets; macOS is out of this release. All native requirements
apply to both supported systems.

## Functional compatibility inventory

| Area                   | Behaviour required in the native application                                                                                                                                                                 | Current reference                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| Projects and scenarios | Selecting, creating, naming, settings, JSON import/export, durable storage, separate main/cleanup graphs                                                                                                     | `Workspace.tsx`, `ProjectSettings.tsx`, `storage.ts`, `tests/project.test.ts`                     |
| Board                  | Moving and multi-selection, pan/zoom, fit view, adding from the palette and menus, deleting blocks and wires, selection linked to the inspector                                                              | `GraphCanvas.tsx`, `graph/operations.ts`                                                          |
| Control flow           | One Start, Next, IF with Yes/No, separate data wires; connecting by port or selector; rejecting cycles and incompatible ports                                                                                | `FlowConnections.tsx`, `tests/workflow.test.ts`                                                   |
| Nested operators       | All existing operators, arguments with typed literals/references/expressions; an operator can replace an argument or be dropped on the board                                                                 | `shared/operators.ts`, `ExpressionBlock.tsx`, `tests/operators.test.ts`                           |
| Comparisons            | One general comparison block, type as an argument operator, no separate "check type" block; comparing any JSON values                                                                                        | `NodeInspector.tsx`, browser test `one comparison handles values and types through its arguments` |
| Variables              | Separate blocks, parameters and environment; dragging onto arguments and URL/query/header/path text fields; a text token and a typed binding have different semantics                                        | `features/variables`, `shared/variables.ts`, `tests/variables.test.ts`                            |
| Response field         | Searchable body/status/headers tree, type and preview, path editing; null/false/0 do not mean absent; sample vs last result distinguished                                                                    | `ResponseBindingPicker.tsx`, `ResponseFieldPicker.tsx`, `responseData.ts`                         |
| Response → variable    | Creating a new variable or setting an existing one to a field reference; dragging a field onto an argument; never persisting a snapshot value instead of a reference                                         | `AssignResponseVariable.tsx`, `variables/operations.ts`                                           |
| Components             | Extracting the selection, explicit public inputs/outputs of any type, boundary mapping, contracts/defaults, graph editing, pinned revisions, explicit instance update                                        | `ComponentBuilder.tsx`, `componentDraft.ts`, `componentDraft.test.ts`                             |
| Copy/paste             | An immutable configuration snapshot at copy time; new IDs for each copy; remapping internal references also inside expressions and IF; external references only to existing sources; blocking a second Start | `graph/operations.test.ts`, the last browser copy test                                            |
| History                | Undo/redo covers the whole change and dependent cleanup references; a drag is one operation, typing merges into edit sessions; separate main/cleanup/component contexts                                      | History in `Workspace.tsx`                                                                        |
| Running                | Simulation/live, full scenario/up to block/single block, cancellation; execution does not block the editor; the result is tied to the input snapshot                                                         | `Workspace.tsx`, `tests/runner.test.ts`, `tests/runner-api.test.ts`                               |
| Report                 | Statuses, duration, request/response, nested component results, main/cleanup, selecting a step leads to the block; stale-result notice after editing                                                         | `RunPanel.tsx`, `redact.ts`                                                                       |
| Report size            | Dragging the separator and keyboard control; constraints after window resize; collapsing keeps the height                                                                                                    | `ResizableRunPanel.tsx`, browser resize test                                                      |
| Secrets                | Names visible in the palette, values only in the secret layer; no values in exports, block clipboard, reports or diagnostics                                                                                 | `EnvironmentEditor.tsx`, `storage.ts`, `redact.test.ts`                                           |

The current 14 browser tests are a specification of user scenarios, not tests that can
run unchanged against egui. Their actions must be reproduced in a native harness, keeping
the existing tests as the behavioural reference during the migration.

## Application state and commands

Split state into four explicit parts:

1. `WorkspaceDocument`: the domain model, projects and versioned components. Serialisable,
   with no egui IDs, runtime handles or secrets. The format version and migrations belong
   to the model/persistence layer.
2. `EditorSession`: document context (project/scenario/phase/draft), selection, board
   camera, panel layout, open field trees, history and graph clipboard. Layout may be
   saved separately from the document.
3. `InteractionState`: the active drag, a connection being started, the modal, field
   focus, working JSON buffers, error messages. This state is never exported.
4. `RunViewState`: run ID, document fingerprint/snapshot, progress, size-bounded results,
   active step, cancellation state. It does not modify the graph when events arrive.

egui components collect intents (`AddNode`, `ConnectData`, `ConnectControl`,
`SetBinding`, `ReplaceExpression`, `PasteFragment`, `CommitComponentRevision`), and a shared
dispatcher validates and applies the transaction. The renderer never edits graph
collections directly. This prevents different connection rules in the palette, inspector
and ports.

A command that changes the model returns the new state or a concrete diagnosis; it never
leaves half a connection. History stores domain transactions or size-bounded snapshots,
not egui state. Node/revision IDs are durable; build widget IDs from the context, the
durable ID and the argument path, never from list position.

A drag moves a preview position; releasing commits one command. Escape restores the
starting position. JSON text may be temporarily invalid: keep a buffer, show the error and
never overwrite a valid document with unparsable text.

The runtime works outside the render thread. The UI takes events from a bounded channel
and has a per-frame handling budget. An event carries the run ID so late responses cannot
overwrite a new run. Cancellation has its own token; closing the window must not wait for
a hanging request on the UI thread. The interface signals ongoing cleanup separately.

## Keyboard, focus and dragging

- A separate shortcut router: modal → active text field → board → application. Ctrl on
  Linux/Windows; rules must not depend on the keyboard layout. The board does not capture
  copy/paste/undo while the user edits text. Escape closes only the topmost active context
  and restores the previous focus.
- The graph clipboard is a typed domain fragment with a snapshot; the system clipboard
  also serves plain text. The first release keeps the current restriction of pasting a
  graph into the same context. No implicit import of references between projects.
- The DnD payload is an enum: block, operator, variable reference or response field. It
  never carries secret values. The target chooses the semantics: a typed argument binding
  or text interpolation. Text fields must deliberately handle the caret position,
  selection and a preview before dropping.
- Every drag operation has a button/menu/selection equivalent. For connections keep the
  form for choosing the next step and the argument source; a port on the drawn graph alone
  is not enough.
- Buttons/ports need stable names, visible focus and a sufficient hit area. Control vs data
  and Yes/No are distinguished by labels/shape too, not by colour alone.

## Board implementation

**Architectural decision:** graph semantics and command rules must be independent of the
drawing library. Choosing a ready-made graph widget or a custom canvas must not change the
project model or require copying the runner into the UI.

| Option                                    | Benefit                                                | Admission condition                                                                                                                                               |
| ----------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Custom canvas over egui Painter + widgets | Full control of labels, ports, nested inputs and focus | Custom hit tests, selection, wires, zoom/pan, clipping and accessibility semantics have tests and a maintenance owner                                             |
| Ready-made egui graph widget              | Less drawing mechanics to maintain                     | Separate verification of licence, project activity, compatibility with the pinned egui and support for Yes/No ports, dynamic inputs, focus control and stable IDs |

The existing viewer uses a custom canvas over egui `Painter`, which is the chosen
direction for the editor.

The board needs one explicit world↔screen transform; ports and hit tests use the same
transform. Zoom anchors under the cursor; fitting covers the selection or the whole
graph. Draw wires under nodes and respect the panel clip area. Show complex text editors
in the inspector or in an edit mode with readable scale; do not assume an arbitrarily
scaled custom drawing gives correct text, IME and accessibility.

Port geometry is a layout result and never enters the model. Changing the number of
arguments or nesting must refresh geometry without camera jumps. For large graphs use
culling of invisible elements, a geometry cache and a bounded number of measurements; also
test a wire crossing the viewport when both ends are outside it.

## Native risks requiring validation

The eframe documentation describes the `accesskit`, `clipboard`, `wayland`, `x11` and
`persistence` features. These are integration capabilities, not proof of correct support
for a particular board. Pin versions compatible with the chosen toolchain and check the
platform matrix before declaring support. Source:
[eframe documentation](https://docs.rs/eframe/latest/eframe/).

1. Screen reader: standard controls and every custom port/node must form a usable
   structure, be selectable without a mouse and announce errors. Enabling AccessKit alone
   does not guarantee this. An alternative step list and a connection form are part of the
   UI design.
2. Linux Wayland/X11 and Windows, HiDPI and two monitors: dragging does not lose state
   outside the panel; clipboard/IME works; scale changes do not move blocks; the separator
   has a readable keyboard alternative. Check shortcuts and non-ASCII characters on both
   systems, and the screen reader on each supported backend separately.
3. Large responses: the JSON tree is searched/expanded with a work limit and
   virtualisation; a multi-megabyte result must not stall rendering. A truncated preview
   must say it is truncated.
4. Restart/crash: the document is saved transactionally outside rendering, save errors are
   visible, external change conflicts and crash recovery have a defined UX. Do not rely on
   egui autosave alone.
5. Sensitive data: disable raw state dumps in production logs; moving from typed
   references to text must not expose secrets. Screenshots/tests use explicit fixtures, not
   the user's environment.

## Stages and acceptance conditions

| Stage                       | UI work                                                                                    | Acceptance evidence                                                                                                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A: contract and reference   | Export current projects/components and behaviour scenarios; list control IDs and shortcuts | Fixture manifest and mapping of every inventory row to a test; import validation does not change references, contracts and revisions                                                                           |
| B: shell and editing        | eframe, layout, inspector, commands, focus, history, save/open                             | Edit → undo/redo → restart yields an identical document; invalid JSON does not corrupt the save; interrupted save test                                                                                         |
| C: board and flow           | Selection, camera, data/control ports, Start/IF, copying groups                            | `graph/operations.test.ts` reproduced; paste keeps the snapshot, internal references and branches; a second Start or a cycle is rejected without partial mutation                                              |
| D: arguments and components | Operators, variables, interpolation, response picking, component editing                   | 0/false/null cases, array/special-key paths, nested operators and extraction of public arguments reproduced; a revision does not change existing instances                                                     |
| E: execution and report     | Runtime events, run modes, cancellation, cleanup, report/tree                              | The HTTP fixture confirms prices → cart → products; an unchosen branch makes no request; the UI responds during slow HTTP; an old run does not overwrite a new one; resize works with mouse and keyboard       |
| F: release                  | Performance, accessibility, packaging and data migration                                   | Native smoke test on Linux Wayland/X11 and Windows, a full mouse-free path, IME/clipboard/DPI/screen reader on both systems, a performance report on agreed machines; clear GPU/file errors and clean shutdown |

Proposed budgets, to be approved on a measurable configuration: board interaction with
500 nodes and 1000 wires without stalls >100 ms, p95 frame time during pan/zoom ≤33 ms;
the UI accepting cancellation ≤100 ms regardless of how long HTTP/cleanup takes. These
are criteria to measure, not currently confirmed parameters.

Do not port automation through screenshot analysis alone. Test command rules without a
GPU; for egui build a harness with deterministic frames and user input and checks of
state/focus. Native clipboard/IME/AccessKit/GPU still need integration tests on a real
backend. Screenshots support layout checks at a fixed font, scale and renderer.
