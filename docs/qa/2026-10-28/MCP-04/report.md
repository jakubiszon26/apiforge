# MCP-04: Save and create from the server — the agent session — QA report

- Branch and commit: `feat/mcp-04-agent-session`. Storage test seam
  `24d16cf`; acceptance tests `305d09b`, criterion 2's fixture corrected in
  `88401f3`; implementation `883c513` (project) and `8818291` (mcp);
  development guide `e08da08`.
- Built with: `cargo build --release --locked -p apiforge-cli` and
  `-p apiforge-desktop` at `e08da08` (release binaries `apiforge` and
  `apiforge-desktop`).
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#63](https://github.com/jakubiszon26/apiforge/issues/63). Ticket:
  [`docs/tickets/MCP-04.md`](../../../tickets/MCP-04.md).
- Machine: Linux 7.0 (Ubuntu 24.04), Rust 1.99.0, rmcp 3.8.0, Python 3.12
  for the stdio client; Xvfb with Lavapipe for the window.

## Linux

The stdio sessions below ran the release binary against a fresh demo-shop
checkout: the managed demo shop created by smoke item 1 (a first start on
empty `XDG_DATA_HOME` and `XDG_CONFIG_HOME`), copied and stripped of
`.apiforge/local`, which is what a fresh Git clone holds; every session got
its own copy. A small Python client drove `apiforge mcp --folder <checkout>`
over stdin and stdout with protocol revision `2025-11-25` and client name
`qa`. In the transcripts `>>` was sent, `<<` was received (cut to about 220
characters), `--` lines are the client's summary of the structured result,
and `##` lines are actions outside the protocol: reading a file, running
`apiforge apply`, capturing the window.

Whether the lock is held is observed from outside with `apiforge apply` of
the saved text: it takes the writer lock, changes nothing, and fails with
*The folder project is open in another writer session* while another
process holds the project.

### Scenario 1: write_source saves exactly, with the checkpoint (criterion 1)

Tests: `mcp04_criterion_1_write_source_saves_exactly_with_the_checkpoint`,
`mcp04_criterion_1_the_same_write_twice_changes_nothing_more`.

Expected: the bytes on disk equal the argument; the checkpoint is written;
`wrote: true`, `changed`, the new `source_sha256`, `generation`, the
diagnostics; every other file untouched; the same files and bytes as a save
in the window.

Observed: as expected. Session 1 below added a comment line to the scenario.
The file equals the argument; `changed` is the scenario and
`.apiforge/diagram.json`, exactly the two files whose hashes changed;
`generation` went 0 → 1. The same text saved by `apiforge apply` in a copy
of the checkout, which saves through the same `ProjectSession::save` as the
window's Ctrl+S, left the scenario, `diagram.json` and the registry
byte-identical (hashes in the transcript). The acceptance test makes the
same comparison with a `ProjectSession` opened as the window opens it, over
the whole shared tree. The comment is reported as a warning (*not attached
to a statement*), which does not stop the save. The same call again answered
`wrote: false`, `changed: []`, the same generation.

### Scenario 2: a text with errors is not saved (criterion 2)

Test: `mcp04_criterion_2_a_text_with_errors_is_not_saved`.

Observed: as expected. A call of a component that does not exist on line 2
of `components/product.afs` answered `invalid_text`, `wrote: false`, two
errors at 2:12–2:27; the file's hash did not change. The test also saves the
KTLO-25 type mismatch, a warning, to show that a warning alone does not
block a save.

### Scenario 3: stale versions (criterion 3)

Tests: `mcp04_criterion_3_a_stale_hash_or_generation_writes_nothing` (the
change in between made by `apply`'s own code) and, over the binary,
`mcp04_criterion_3_a_write_based_on_a_version_apply_replaced_is_stale`
(the real `apiforge apply` between `read_source` and `write_source`).

Observed: as expected. In session 1 a write based on the hash read before
the first write answered `stale_source` with the current hash `aada8d2b…`;
a write naming generation 0 after the head reached 1 answered
`stale_project` with `generation: 1`. Nothing was written either time.

### Scenario 4: a new document is checked first (criterion 4)

Tests:
`mcp04_criterion_4_check_new_document_compares_the_kind_and_writes_nothing`,
`mcp04_criterion_4_create_document_refuses_on_any_error`.

Observed: as expected. Over stdio: the same document again is
`path_exists` with the file's `source_sha256`, so a retry after a lost
answer can tell; the demo's product text at a new path is `handle_exists`;
`../outside.afs` is `path_rejected`; a scenario created as a component is
`invalid_text` (*The header declares another kind of document*); a scenario
calling `nosuch@1` is `invalid_text` with the errors at 2:7. The test
checks, for ten refusals (registered path, case collision, file on disk,
case collision on disk, taken handle, `../`, `.apiforge/`, `.txt`, another
kind, errors), that the shared tree is byte-identical afterwards: no source,
no registry entry, no checkpoint. The kind comparison of
`check_new_document` already existed (MCP-03); its test passed before the
implementation.

### Scenario 5: one transaction for a new document (criterion 5)

Tests:
`mcp04_criterion_5_create_document_commits_everything_in_one_transaction`,
`mcp04_criterion_5_a_fault_before_activation_installs_nothing`.

Observed: as expected. Session 1 created `components/discount.afs`:
`changed` names the source, `diagram.json` and `apiforge.project.json`; the
head generation went 1 → 2, one advance; the registry entry is a component;
the checkpoint record's `sourceHash` is the file's hash. The test then opens
the project as the window does and finds nothing left to seal:
`diagram.json` and the generation do not change. With the storage's I/O
seam failing the second staged file, the call answered an error with
`wrote: false`, the head stayed inactive at the same generation and the
shared tree was byte-identical; the next call created the document.

### Scenario 6: a failure after activation (criterion 6)

Tests: `mcp04_criterion_6_a_failure_after_activation_is_recovery_required`;
in the window, `the_window_finishes_a_creation_that_failed_after_activation`
(`crates/apiforge-desktop/src/folder/acceptance_tests.rs`, `mod mcp04`).

Expected: `recovery_required` with `wrote: "unknown"`; every later write of
the session the same code; the window's recovery view finishes the
operation once it holds the project.

Observed: as expected, in the tests only. The seam fails the install of
`diagram.json` after activation: the answer is `recovery_required`,
`wrote: "unknown"`, and the head keeps its active transaction. A later
`write_source` and a later `create_document` are refused with
`recovery_required` and `wrote: false`, since those calls did nothing, and
the files stay as they were; the server does not repair. After
`finish_editing`, the headless window opens on *Recover interrupted file
operation*; *Finish intended operation*, then *Open project*, shows the new
component registered with its text, and `diagram.json` holds its record. A
fault cannot be injected into the release binary, so this scenario has no
stdio transcript.

### Scenario 7: the agent session holds the lock (criterion 7)

Tests: `mcp04_criterion_7_the_session_holds_the_lock_until_finish_editing`;
over the binary,
`mcp04_criterion_7_the_session_keeps_the_lock_until_eof_and_names_the_client`
(the client name from `initialize`) and
`mcp04_criterion_7_finish_editing_releases_the_lock_over_stdio` (protocol
revision `2026-07-28`, the client name from the request metadata).

Observed: as expected. In session 1, after `write_source` returned, the
holder file read
`{"version":1,"program":"apiforge","role":"agent","client":"qa","since":"2026-10-28T00:57:44Z","endpoint":null}`
and `apiforge apply` failed with *The folder project is open in another
writer session*. `project_overview` reported `holder: "agent_session"`.
`finish_editing` answered `finished: true`; `apiforge apply` 0.028 s later
succeeded, and the overview then said `holder: "none"`. In session 2 the
client closed stdin after a write: `apiforge apply` succeeded 0.027 s later
and the server exited with code 0. The tests assert the release within one
second, and that a 300 ms pause between calls keeps the lock.

The window opened beside the session (captured during session 1, after the
first write) is read-only with MCP-07's bar *qa via apiforge mcp · since
00:57 · editing is paused here; this window follows every change*, and shows
the model's comment on line 2:
[observing-960x540.png](observing-960x540.png),
[observing-1600x1000.png](observing-1600x1000.png).

### Scenario 8: another process holds the project (criterion 8)

Tests:
`mcp04_criterion_8_a_write_while_another_process_holds_the_project_is_refused`
(a window, `apiforge apply`, another agent),
`mcp04_criterion_8_a_holder_without_a_description_is_held_by_other`; over
the binary,
`mcp04_criterion_8_a_window_holding_the_project_refuses_the_write_over_stdio`.

Observed: as expected. In session 4 a real `apiforge-desktop --folder`
window held the checkout. `write_source` and `create_document` answered
`held_by_window`, `wrote: false`: *Another API Forge window has held this
project since 2026-10-28T00:57:47Z and saves it there; nothing was written.
Ask the person to close the project in that window, then write again;
reading works meanwhile*, with `details.holder`. `check` in the same session
answered normally, and no file changed. The tests check `held_by_other` for
`apiforge apply` (*apiforge apply has held…*), another agent (*Other client
via apiforge mcp has held…*) and a holder without a description (*Another
process holds this project…*), and that no message advises editing the file
directly.

### Scenario 9: a known secret in the text (criterion 9)

Tests: `mcp04_criterion_9_a_known_secret_in_the_text_is_refused`; over the
binary,
`mcp04_criterion_9_a_secret_from_the_environment_is_refused_and_never_echoed`.

Observed: as expected. Session 3 started the server with
`APIFORGE_SECRET_TOKEN` set; a write and a creation whose text contained the
value answered `secret_in_text`, `wrote: false`. The value appeared in no
response and not on stderr, and no file changed.

### Scenario 10: identities survive (criterion 10)

Test: `mcp04_criterion_10_identities_survive_a_server_save`.

Observed: passes. The server adds a request block `ping` to the scenario;
the project opened with `ProjectSession::open` has every earlier block with
the same id, kind and position, and `ping` has the id and position of its
record in `diagram.json`; opening the project changes no byte of
`diagram.json`. Not exercised by hand in the window.

### Scenario 11: a read-only window beside the server (criterion 11)

Tests (headless `egui_kittest`):
`a_read_only_window_follows_a_write_on_a_clean_document`,
`a_write_on_a_document_unsaved_in_the_window_keeps_the_unsaved_text`.

Observed: pass. The window, opened while the session holds the project, is
read-only; after a `write_source` and one scan it shows the new text with no
conflict and no error notice. With unsaved text in the window, the same scan
shows *Changed outside API Forge while unsaved here* and keeps the unsaved
text; the disk holds the model's text. The capture of scenario 7 shows the
release window on a project the server wrote (opened after the write); a
running window following a write was not watched by hand.

### Verification (criterion 12)

At `e08da08`, Linux:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: no
  warning.
- `cargo test --workspace --all-targets --locked`: 1 221 passed, 0 failed,
  30 ignored over 59 test binaries.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit 0.
- `cargo run -p apiforge-project --example analysis --release --locked`:
  parse, lower and validate of 1 000 lines in 1.67 ms (budget 5 ms). The
  analysis itself is unchanged; the session gained two methods.

The scenario language, the layout and the code view are untouched, so the
fuzzers, the layout bench and the frame budgets were not run.

### Transcripts

#### Session 1: the agent session, write_source, create_document, finish_editing

```text
>> {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "qa", "version": "1.0.0"}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"resources":{},"tools":{}},"serverInfo":{"name":"apiforge","version":"0.1.0"},"instructions":"API Forge project tools. Every document is na …
>> {"jsonrpc": "2.0", "method": "notifications/initialized"}
>> {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
<< {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"project_overview","description":"What the API Forge project contains: name, base URL, environment variable and secret names, contracts, every document (kind, registered …
-- tool project_overview: readOnly=True destructive=None idempotent=None
-- tool language_reference: readOnly=True destructive=None idempotent=None
-- tool read_source: readOnly=True destructive=None idempotent=None
-- tool check: readOnly=True destructive=None idempotent=None
-- tool check_text: readOnly=True destructive=None idempotent=None
-- tool print: readOnly=True destructive=None idempotent=None
-- tool run: readOnly=True destructive=None idempotent=None
-- tool write_source: readOnly=False destructive=True idempotent=True
-- tool create_document: readOnly=False destructive=False idempotent=False
-- tool finish_editing: readOnly=False destructive=False idempotent=True
>> {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "read_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs"}}}
<< {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"scenarios/promotion-and-cart-price.afs\",\n  \"text\": \"scenario \\\"Promotion and cart price\\\" {\\n  description \\\"Check the prom …
-- read_source: source_sha256=fb9612f8f143258b92c96162bb3f945d74101b9294a7d4defd2b2d71693cdb16 generation=0 valid=True
>> {"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs", "text": "scenario \"Promotion and cart price\" {\n  // Checked by the …
<< {"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"scenarios/promotion-and-cart-price.afs\",\n  \"wrote\": true,\n  \"changed\": [\n    \"scenarios/promotion-and-cart-price.afs\",\n    \ …
-- write_source: wrote=true changed=["scenarios/promotion-and-cart-price.afs", ".apiforge/diagram.json"] source_sha256="aada8d2ba794e44c41a81ca536e8ef1060e7df2b7174153e286bba2112dea064" generation=1 valid=true
-- on disk: scenarios/promotion-and-cart-price.afs equals the argument: True; head generation 1
-- files changed: .apiforge/diagram.json, scenarios/promotion-and-cart-price.afs
## .apiforge/local/holder.json: {"version":1,"program":"apiforge","role":"agent","client":"qa","since":"2026-10-28T00:57:44Z","endpoint":null}
## apiforge apply while the session holds the project: exit 1: The folder project is open in another writer session.
## the same text saved by apiforge apply in a copy: exit 0; scenario, diagram.json and registry byte-identical: True
##   aada8d2ba794e44c41a81ca536e8ef1060e7df2b7174153e286bba2112dea064  scenarios/promotion-and-cart-price.afs (server)
##   aada8d2ba794e44c41a81ca536e8ef1060e7df2b7174153e286bba2112dea064  scenarios/promotion-and-cart-price.afs (apply)
##   25e1902e3f829ed5e3da1c2fab71c9b37f8643104889cb36412cf795f1e033d5  .apiforge/diagram.json (server)
##   25e1902e3f829ed5e3da1c2fab71c9b37f8643104889cb36412cf795f1e033d5  .apiforge/diagram.json (apply)
## screenshot observing-960x540.png: exit 0
## screenshot observing-1600x1000.png: exit 0
>> {"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs", "text": "scenario \"Promotion and cart price\" {\n  // Checked by the …
<< {"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"scenarios/promotion-and-cart-price.afs\",\n  \"wrote\": false,\n  \"changed\": [],\n  \"source_sha256\": \"aada8d2ba794e44c41a81ca536e8 …
-- write_source again: wrote=false changed=[] generation=1
>> {"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs", "text": "scenario \"Promotion and cart price\" {\n  description \"Che …
<< {"jsonrpc":"2.0","id":6,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"stale_source\",\n  \"message\": \"The document changed on disk since the version you read; nothing was written. Read it again, merge yo …
-- write_source from the old version: isError code=stale_source wrote=false message='The document changed on disk since the version you read; nothing was written. Read it again, merge your change and write with the new source_sha256' {"source_sha256": "aada8d2ba794e44c41a81ca536e8ef1060e7df2b7174153e286bba2112dea064"}
>> {"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs", "text": "scenario \"Promotion and cart price\" {\n  // Checked by the …
<< {"jsonrpc":"2.0","id":7,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"stale_project\",\n  \"message\": \"The project changed since the generation you checked at; nothing was written. Check again and write  …
-- write_source at the old generation: isError code=stale_project wrote=false message='The project changed since the generation you checked at; nothing was written. Check again and write with the new generation' {"generation": 1}
>> {"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "components/product.afs", "text": "component product \"Product\" {\n  broken = nosuch@1(id: 1)\n  description \ …
<< {"jsonrpc":"2.0","id":9,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"invalid_text\",\n  \"message\": \"The text has errors and was not saved; fix the diagnostics and write again\",\n  \"wrote\": false,\n  …
-- write_source with an error: isError code=invalid_text wrote=false message='The text has errors and was not saved; fix the diagnostics and write again' {"diagnostics": ["2:12-2:27 error Operator calls cannot name a revision", "2:12-2:27 error Unknown operator or nested component call"]}
-- components/product.afs unchanged: True
>> {"jsonrpc": "2.0", "id": 10, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "component", "path": "components/discount.afs", "text": "component discount \"Discount\" {\n  description \ …
<< {"jsonrpc":"2.0","id":10,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"components/discount.afs\",\n  \"kind\": \"component\",\n  \"handle\": \"discount\",\n  \"wrote\": true,\n  \"changed\": [\n    \"compo …
-- create_document: path="components/discount.afs" kind="component" handle="discount" wrote=true changed=["components/discount.afs", ".apiforge/diagram.json", "apiforge.project.json"] source_sha256="0217beee7cdab9185bc7477a41ed117e40e5dce011088909a290ed9c3d8b44c1" generation=2
-- head generation 1 -> 2; registry entry kind=component; checkpoint record sourceHash=0217beee7cdab9185bc7477a41ed117e40e5dce011088909a290ed9c3d8b44c1; file equals the argument: True
>> {"jsonrpc": "2.0", "id": 11, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "component", "path": "components/discount.afs", "text": "component discount \"Discount\" {\n  description \ …
<< {"jsonrpc":"2.0","id":11,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"path_exists\",\n  \"message\": \"A document is already registered under this path or one differing only in case\",\n  \"wrote\": false …
-- create_document again: isError code=path_exists wrote=false message='A document is already registered under this path or one differing only in case' {"source_sha256": "0217beee7cdab9185bc7477a41ed117e40e5dce011088909a290ed9c3d8b44c1", "problems": [{"code": "path_exists", "message": "A document is already registered under this path or one differing only in case"}, {"code": "handle_exists", "message": "Another component already uses this handle"}], "diagnostics": []}
>> {"jsonrpc": "2.0", "id": 12, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "component", "path": "components/copy.afs", "text": "component product \"Product\" {\n  description \"Fetch …
<< {"jsonrpc":"2.0","id":12,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"handle_exists\",\n  \"message\": \"Another component already uses this handle\",\n  \"wrote\": false,\n  \"details\": {\n    \"problem …
-- create_document, taken handle: isError code=handle_exists wrote=false message='Another component already uses this handle' {"source_sha256": null, "problems": [{"code": "handle_exists", "message": "Another component already uses this handle"}], "diagnostics": ["1:1-18:1 error The definition is not valid in this project"]}
>> {"jsonrpc": "2.0", "id": 13, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "scenario", "path": "../outside.afs", "text": "scenario \"Draft\" {\n}\n"}}}
<< {"jsonrpc":"2.0","id":13,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"path_rejected\",\n  \"message\": \"Invalid project-relative path.\",\n  \"wrote\": false,\n  \"details\": {}\n}"}],"structuredContent" …
-- create_document, ../: isError code=path_rejected wrote=false message='Invalid project-relative path.' {}
>> {"jsonrpc": "2.0", "id": 14, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "component", "path": "components/draft.afs", "text": "scenario \"Draft\" {\n}\n"}}}
<< {"jsonrpc":"2.0","id":14,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"invalid_text\",\n  \"message\": \"The header declares another kind of document\",\n  \"wrote\": false,\n  \"details\": {\n    \"proble …
-- create_document, other kind: isError code=invalid_text wrote=false message='The header declares another kind of document' {"source_sha256": null, "problems": [{"code": "invalid_text", "message": "The header declares another kind of document"}], "diagnostics": ["1:1-3:1 error This code describes a different kind of document"]}
>> {"jsonrpc": "2.0", "id": 15, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "scenario", "path": "scenarios/broken.afs", "text": "scenario \"Broken\" {\n  x = nosuch@1(id: 1)\n}\n"}}}
<< {"jsonrpc":"2.0","id":15,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"invalid_text\",\n  \"message\": \"The text has errors and was not saved; fix the diagnostics and create it again\",\n  \"wrote\": fals …
-- create_document, errors: isError code=invalid_text wrote=false message='The text has errors and was not saved; fix the diagnostics and create it again' {"source_sha256": null, "problems": [], "diagnostics": ["2:7-2:22 error Operator calls cannot name a revision", "2:7-2:22 error Unknown operator or nested component call"]}
-- project_overview holder=agent_session generation=2
>> {"jsonrpc": "2.0", "id": 17, "method": "tools/call", "params": {"name": "finish_editing", "arguments": {}}}
<< {"jsonrpc":"2.0","id":17,"result":{"content":[{"type":"text","text":"{\n  \"finished\": true\n}"}],"structuredContent":{"finished":true},"isError":false}}
-- finish_editing: finished=True; ## apiforge apply 0.028 s later: exit 0
-- project_overview holder=none
-- EOF: exit code 0 after 0.003 s; stdout lines 18, non-protocol lines 0
-- stderr: apiforge mcp: write_source: stale_source | apiforge mcp: write_source: stale_project | apiforge mcp: write_source: invalid_text | apiforge mcp: create_document: path_exists | apiforge mcp: create_document: handle_exists | apiforge mcp: create_document: path_rejected | apiforge mcp: create_document: invalid_text | apiforge mcp: create_document: invalid_text
```

Files of the checkout before and after the session (SHA-256; `.apiforge/local` left out):

```text
546ce0a430a5  .apiforge/diagram.json  (changed; before 8c1e95575605)
b07c465301bc  .apiforge/revisions/active-promotion/1.afs  (unchanged; before b07c465301bc)
eba04ff2905f  .apiforge/revisions/get-product/1.afs  (unchanged; before eba04ff2905f)
1cf5820dbbde  .apiforge/revisions/product-cart/1.afs  (unchanged; before 1cf5820dbbde)
613bb3a728cf  .gitattributes  (unchanged; before 613bb3a728cf)
f3652585d78c  .gitignore  (unchanged; before f3652585d78c)
41a49bdeb19b  apiforge.project.json  (changed; before cef54d547d4c)
b07c465301bc  components/active-promotion.afs  (unchanged; before b07c465301bc)
1cf5820dbbde  components/add-to-cart.afs  (unchanged; before 1cf5820dbbde)
0217beee7cda  components/discount.afs  (changed; before ------------)
eba04ff2905f  components/product.afs  (unchanged; before eba04ff2905f)
aada8d2ba794  scenarios/promotion-and-cart-price.afs  (changed; before fb9612f8f143)
```

#### Session 2: EOF ends the agent session

```text
>> {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "qa", "version": "1.0.0"}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"resources":{},"tools":{}},"serverInfo":{"name":"apiforge","version":"0.1.0"},"instructions":"API Forge project tools. Every document is na …
>> {"jsonrpc": "2.0", "method": "notifications/initialized"}
>> {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs", "text": "scenario \"Promotion and cart price\" {\n  // Checked by the …
<< {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"scenarios/promotion-and-cart-price.afs\",\n  \"wrote\": true,\n  \"changed\": [\n    \"scenarios/promotion-and-cart-price.afs\",\n    \ …
-- write_source: wrote=true generation=1
## apiforge apply while the session holds the project: exit 1
## apiforge apply after EOF: exit 0, 0.027 s after stdin closed
-- server exit code 0
```

#### Session 3: a known secret in the text

```text
## APIFORGE_SECRET_TOKEN set in the server's environment
>> {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "qa", "version": "1.0.0"}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"resources":{},"tools":{}},"serverInfo":{"name":"apiforge","version":"0.1.0"},"instructions":"API Forge project tools. Every document is na …
>> {"jsonrpc": "2.0", "method": "notifications/initialized"}
>> … "name":"write_source","arguments":{"path":"scenarios/promotion-and-cart-price.afs","text":"… // token <the secret> …", …}
-- write_source: isError code=secret_in_text wrote=false message='The text contains the value of a known secret; nothing was written and the value is not repeated. Name the secret instead: env.name' {}
>> … "name":"create_document","arguments":{"kind":"scenario","path":"scenarios/leak.afs","text":"… <the secret> …"}
-- create_document: isError code=secret_in_text wrote=false message='The text contains the value of a known secret; nothing was written and the value is not repeated. Name the secret instead: env.name' {}
-- EOF: exit code 0 after 0.001 s; stdout lines 4, non-protocol lines 0
-- stderr: apiforge mcp: write_source: secret_in_text | apiforge mcp: create_document: secret_in_text
-- the secret in any response after the reads: False; in stderr: False
-- files unchanged: True
```

#### Session 4: a window holds the project

```text
## apiforge-desktop --folder <checkout> started; holder.json: {"version":1,"program":"apiforge","role":"window","client":null,"since":"2026-10-28T00:57:47Z","endpoint":null}
>> {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "qa", "version": "1.0.0"}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"resources":{},"tools":{}},"serverInfo":{"name":"apiforge","version":"0.1.0"},"instructions":"API Forge project tools. Every document is na …
>> {"jsonrpc": "2.0", "method": "notifications/initialized"}
>> {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "write_source", "arguments": {"path": "scenarios/promotion-and-cart-price.afs", "text": "scenario \"Promotion and cart price\" {\n  // Checked by the …
<< {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"held_by_window\",\n  \"message\": \"Another API Forge window has held this project since 2026-10-28T00:57:47Z and saves it there; nothi …
-- write_source: isError code=held_by_window wrote=false message='Another API Forge window has held this project since 2026-10-28T00:57:47Z and saves it there; nothing was written. Ask the person to close the project in that window, then write again; reading works meanwhile' {"holder": {"role": "window", "client": null, "since": "2026-10-28T00:57:47Z"}}
>> {"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "create_document", "arguments": {"kind": "scenario", "path": "scenarios/draft.afs", "text": "scenario \"Draft\" {\n}\n"}}}
<< {"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"held_by_window\",\n  \"message\": \"Another API Forge window has held this project since 2026-10-28T00:57:47Z and saves it there; nothi …
-- create_document: isError code=held_by_window wrote=false message='Another API Forge window has held this project since 2026-10-28T00:57:47Z and saves it there; nothing was written. Ask the person to close the project in that window, then write again; reading works meanwhile' {"holder": {"role": "window", "client": null, "since": "2026-10-28T00:57:47Z"}}
>> {"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "check", "arguments": {}}}
<< {"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"{\n  \"generation\": 0,\n  \"valid\": true,\n  \"documents\": [\n    {\n      \"path\": \"scenarios/promotion-and-cart-price.afs\",\n      \"source_sha2 …
-- check in the same session: isError=False valid=True
-- EOF: exit code 0 after 0.003 s; stdout lines 5, non-protocol lines 0
-- stderr: apiforge mcp: write_source: held_by_window | apiforge mcp: create_document: held_by_window
-- files unchanged: True
```
## Windows

From CI run 40612919252 of this branch at `116aa00` (windows-2022), the
code before review round 1:

- `cargo test --workspace --all-targets --locked`: 1 210 passed, 0 failed,
  30 ignored over 59 test binaries. The tests that run on Linux and not on
  Windows are Unix symlink and permission tests of other tickets. All 22
  MCP-04 tests passed on Windows: the 14 library tests, among them the lock
  held across calls and released within a second (Windows' mandatory
  `LockFileEx` lock), the holder file, `held_by_window` and the two fault
  injections; the 5 stdio sessions of the binary (EOF, `finish_editing`
  with protocol `2026-07-28`, the real `apiforge apply`, the secret from
  the environment, a window holding the project); and the 3 headless window
  tests of criteria 6 and 11.
- The first run, 40611429880 at `e08da08`, failed six of the library tests
  on Windows only. Their helper listed the project by reading every file,
  the lock file in `.apiforge/local` among them, and Windows keeps that file
  locked while the session holds it. Since `116aa00` the helper does not
  read that folder. The product never reads the lock file.
- The folder-project and managed-startup step passed. Its three captures at
  960 × 540 have the standard size; the three named 1600 × 1000 are
  1024 × 768, because the runner's display is smaller (a warning on the run,
  as the testing guide describes). The Code capture at 960 × 540 shows the
  managed demo shop in the runner's Polish locale, open with "No problems":
  [windows-code-960x540.png](windows-code-960x540.png). Nothing on screen
  changes in this ticket; the capture shows that the window still starts.
- The stdio transcripts, the captures of the window beside a session and
  the smoke list were not run on Windows by hand; the owner's Windows run is
  pending. The fixes of review round 1 run on Windows in this pull request's
  CI run for its head.

## Not verified

- **Claude Code with the write tools: not verified by the implementing
  agent; left to the owner.** With the release binary registered as in the
  guide, `/mcp` should list `write_source`, `create_document` and
  `finish_editing`; a write should show the window's bar *Claude Code via
  apiforge mcp*.
- A fault after activation through the release binary (scenario 6): only
  the tests inject it, through the storage seam, which no release build
  has.
- A running window following a write live: covered headlessly by the
  criterion 11 tests; the release capture shows a window opened after a
  write.
- The hand-over from a window (MCP-08b) and the observer's reload (MCP-08a):
  not in scope. Today a write while the window holds the project is refused.

## Regression smoke list

Linux, 2026-10-28, release binaries of `e08da08`, on Xvfb (`xvfb-run`, X11
backend, Lavapipe), with empty scratch `XDG_DATA_HOME` and
`XDG_CONFIG_HOME`.

| # | Result |
| --- | --- |
| 1 | Pass: `apiforge-desktop` with no arguments created and opened the managed demo shop under the scratch data directory (`apiforge/projects/shop`, `recent-projects.json`). Ended by `timeout` after 15 s; nothing on stderr. |
| 2 | Pass: `--screenshot … --folder <checkout> --mode code` at 960 × 540 and 1600 × 1000 showed the explorer tree and the scenario open in Code; exit 0 (the captures of scenario 7). |
| 7 | Pass for the bytes: the server's save and `apiforge apply`'s save of the same text are byte-identical, and the project reopens with the same identities (scenario 10). Ctrl+S in the window: not run, no window change. |
| 3–6, 8–10 | Not run: no interface change. The workspace suite that covers them passes. |

## Review round 1

The automated review of PR #111 (cubic) found five points and CodeQL eight
cleartext-logging alerts, all in the acceptance tests. Every point is fixed,
in `7171ea1` (project), `b48e3b4` (mcp) and `e752b2d` (guide):

- **The overview waited for a write.** Whether the session holds the
  project is now an atomic flag, so `project_overview` answers during a
  write.
- **A parent folder in another letter case.** `Components/x.afs` beside
  `components/` is now `path_exists` from `check_text` and
  `create_document`, not an I/O error from the commit. Test:
  `mcp04_review_a_folder_in_another_letter_case_is_path_exists`.
- **Conflicts.** A file another program changed while a write was prepared
  is reported by what changed: the document `stale_source`, a new
  document's path `path_exists`, the checkpoint or registry `stale_project`.
  Test: `mcp04_review_a_file_changed_during_the_commit_names_what_changed`,
  which also reads the active flag from inside the commit.
- **`wrote: true` on an error.** It is no longer representable; the schema
  allows `false` or `"unknown"`.
- **A cloned diagnostics vector.** The check takes it by value.
- **CodeQL.** The alerts were assertion messages that printed a tool's
  error message on failure. The messages are static and never carry a
  secret, which the tests show. The tests no longer print them.

Before the review, the first CI run (40611429880) failed six tests on
Windows only. The tests' helper read the lock file inside `.apiforge/local`,
which Windows keeps locked while the session holds it. Since `116aa00` the
helper does not read that folder. The product was not affected: it never
reads the lock file.

These fixes change neither the files written nor the tool results of the
transcripts above, which were taken at `e08da08`, with two exceptions. A
`Conflict` is now named by what changed, and `path_exists` now also covers a
folder in another letter case; neither occurs in the sessions above.

Verification at `e752b2d`, Linux: `cargo fmt --all -- --check` clean;
clippy with `-D warnings` no warning; `cargo test --workspace --all-targets
--locked` 1 223 passed, 0 failed, 30 ignored over 59 test binaries; the
fixture check exit 0. Windows: the CI run of the pushed head.
