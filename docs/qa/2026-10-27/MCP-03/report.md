# MCP-03: Run a simulation, check a proposal, serve the reference and resources — QA report

- Branch and commit: `feat/mcp-03-run-check-text`. Acceptance tests
  `ae1cb14`; implementation `44bb3db` (script), `a07d541` (project),
  `9627ee4` (mcp), `91f9618` (cli); development guide `3824480`; merge of
  `origin/main` (MCP-07) `013ccfe`; Windows line-ending fix of the criterion 9
  fixture `468d16b`; the end-of-stdin wording in the guide `9468081`.
- Built with: `cargo build --release --locked -p apiforge-cli -p
  apiforge-desktop` at `013ccfe` (release binaries `apiforge` and
  `apiforge-desktop`). The two later commits change `.gitattributes`, the
  guide and a doc comment only, not what is built.
- Mock-up: not applicable; nothing on screen changes. Issue:
  [#62](https://github.com/jakubiszon26/apiforge/issues/62). Ticket:
  [`docs/tickets/MCP-03.md`](../../../tickets/MCP-03.md).
- Machine: Linux 7.0 (Ubuntu 24.04), Rust 1.99.0, rmcp 3.8.0, Python 3.12
  for the stdio client; Xvfb with Lavapipe for the window.

## Linux

The stdio sessions below ran the release binary against a fresh demo-shop
checkout: the managed demo shop created by smoke item 1 (a first start on
empty `XDG_DATA_HOME` and `XDG_CONFIG_HOME`), copied and stripped of
`.apiforge/local`, which is what a fresh Git clone holds. A small Python
client drove `apiforge mcp --folder <demo>` over stdin and stdout with
protocol revision `2025-11-25`. In the transcripts `>>` was sent, `<<` was
received (cut to about 220 characters), and `--` lines are the client's
summary of the structured result.

### Files before and after (criteria 1, 2, 4, 7, 12)

SHA-256 of every file of the checkout, and the list of every path, were taken
before the four sessions (tools, cancel, deadline, EOF) and after them. They
are identical, and `.apiforge/local` was not created:

```text
ddf77b7b23294fe3678778a05a20d39ca8e1144c79f6e93765786fa104a39c68  ./.apiforge/diagram.json
b07c465301bc91eae964ab3f203a5119b3fa83ebc5a26c5d82b40ae58be8081d  ./.apiforge/revisions/active-promotion/1.afs
eba04ff2905ff3d655393752a159701cb261aca977c76eb5b941b41b7f54cdef  ./.apiforge/revisions/get-product/1.afs
1cf5820dbbdecbeaf1d0dc59d9a8f5e35e82f2cb5dcaea72cf6e372204b66e13  ./.apiforge/revisions/product-cart/1.afs
613bb3a728cf699b5b3826f9ff60e863d3485c7dce49ddf17b7ef7ffa412d3a0  ./.gitattributes
f3652585d78c7c35f63fc147ed1d7bf97f68e565d8292069a9b9d24bc0e0e89f  ./.gitignore
306609c070a11e4318db703e3ca51a4d6365d68261152114b423bc95d304014a  ./apiforge.project.json
b07c465301bc91eae964ab3f203a5119b3fa83ebc5a26c5d82b40ae58be8081d  ./components/active-promotion.afs
1cf5820dbbdecbeaf1d0dc59d9a8f5e35e82f2cb5dcaea72cf6e372204b66e13  ./components/add-to-cart.afs
eba04ff2905ff3d655393752a159701cb261aca977c76eb5b941b41b7f54cdef  ./components/product.afs
fb9612f8f143258b92c96162bb3f945d74101b9294a7d4defd2b2d71693cdb16  ./scenarios/promotion-and-cart-price.afs
```

### The tools session

```text
>> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"qa","version":"1.0.0"}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"resources":{},"tools":{}},"serverInfo":{"name":"apiforge","version":"0.1.0"},"instructions":"API Forge project tools. …"}}
>> {"jsonrpc":"2.0","method":"notifications/initialized"}
>> {"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
<< {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"project_overview","description":"What the API Forge project contains: …"}, …]}}
-- tools: project_overview, language_reference, read_source, check, check_text, print, run (each readOnlyHint)
>> … "name":"check_text","arguments":{"path":"components/product.afs","text":"component product \"Product\" { … export ??? …"}
-- check_text registered: registered=True valid=False first=error 16:10 'Expected an identifier'; generation=0 used=3 source_sha256=eba04ff2905f…
>> … "name":"check_text","arguments":{"path":"components/new-product.afs","text":"component new_product \"Product\" { …"}
-- check_text new: registered=False kind=component handle=new_product valid=True problems=[]
>> … "name":"check_text","arguments":{"path":"components/copy.afs","text":"component product \"Product\" { …"}
-- check_text taken handle: valid=False problems=['handle_exists']
>> … "name":"check_text","arguments":{"path":"components/headless.afs","text":"hello = 1\n"}
-- check_text no kind: kind=None problems=['invalid_text']
>> … "name":"check_text","arguments":{"path":"scenarios/calls.afs","text":"scenario \"Calls\" {\n  next = product@2(id: 123)\n  other = nosuch@1(id: 1)\n  missing = GET \"/x/{{env.MISSING}}\" …"}
-- check_text names: 2:10 Component revision does not exist; 2:24 Unknown component input; 3:11 Operator calls cannot name a revision; 3:11 Unknown operator or nested component call; 4:23 Unknown environment name
>> … "name":"run","arguments":{"path":"scenarios/promotion-and-cart-price.afs"}
<< {"jsonrpc":"2.0","id":8,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"scenarios/promotion-and-cart-price.afs\",\n  \"status\": \"passed\",\n  \"ended_by\": \"completed\", …"}], …}}
-- run scenario: status=passed ended_by=completed step_count=18 redacted=True top-level steps=8 compact text=3448 chars
>> … "name":"run","arguments":{"path":"scenarios/promotion-and-cart-price.afs","full":true}
-- run full: report.status=passed mode=simulation steps=8 step_count=18
>> … "name":"run","arguments":{"path":"components/product.afs","inputs":{"id":7}}
-- run component id=7: status=passed steps=['fetch_product', 'product']
>> … "name":"run","arguments":{"path":"components/active-promotion.afs"}
-- run component without inputs: code=invalid_inputs details={'missing': ['product'], 'unknown': [], 'invalid': [], 'inputs': [{'name': 'product', 'type': 'object', 'has_default': False, 'optional': False}]}
>> … "name":"run","arguments":{"path":"components/product.afs","inputs":{"nope":1}}
-- run unknown input: code=invalid_inputs details={'missing': [], 'unknown': ['nope'], 'invalid': [], 'inputs': [{'name': 'id', 'type': 'number', 'has_default': True, 'optional': False}]}
>> … "name":"run","arguments":{"path":"components/missing.afs"}
-- run unregistered: code=unknown_document
>> … "name":"run","arguments":{"path":"scenarios/promotion-and-cart-price.afs","live":true}
<< {"jsonrpc":"2.0","id":14,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"live_disabled\",\n  \"message\": \"live requests are not enabled on this server\",\n  \"wrote\": false,\n  \"details\": {}\n}"}], …}}
>> … "name":"language_reference","arguments":{}
-- language_reference: version=1 27792 chars
>> … "name":"project_overview","arguments":{}
-- overview reference_version=1
>> {"jsonrpc":"2.0","id":17,"method":"resources/list","params":{}}
-- resources: apiforge://reference, apiforge://source/scenarios/promotion-and-cart-price.afs, apiforge://source/components/add-to-cart.afs, apiforge://source/components/product.afs, apiforge://source/components/active-promotion.afs
>> {"jsonrpc":"2.0","id":18,"method":"resources/read","params":{"uri":"apiforge://reference"}}
-- read reference: mimeType=text/plain
>> {"jsonrpc":"2.0","id":19,"method":"resources/read","params":{"uri":"apiforge://source/components/product.afs"}}
-- read source: mimeType=text/plain equals file=True
>> {"jsonrpc":"2.0","id":20,"method":"resources/read","params":{"uri":"apiforge://source/%2e%2e/x.afs"}}
<< {"jsonrpc":"2.0","id":20,"error":{"code":-32602,"message":"Invalid project-relative path.","data":{"code":"path_rejected","message":"Invalid project-relative path."}}}
>> {"jsonrpc":"2.0","id":21,"method":"resources/read","params":{"uri":"apiforge://source/.apiforge/diagram.json"}}
<< {"jsonrpc":"2.0","id":21,"error":{"code":-32602,"message":"Invalid project-relative path.","data":{"code":"path_rejected","message":"Invalid project-relative path."}}}
>> {"jsonrpc":"2.0","id":22,"method":"resources/read","params":{"uri":"apiforge://source/components/missing.afs"}}
<< {"jsonrpc":"2.0","id":22,"error":{"code":-32002,"message":"No registered document has this path","data":{"code":"unknown_document","message":"No registered document has this path"}}}
-- EOF: exit code 0 after 0.003 s; stdout lines 22, non-protocol lines 0
-- stderr: apiforge mcp: run: invalid_inputs | apiforge mcp: run: invalid_inputs | apiforge mcp: run: unknown_document | apiforge mcp: run: live_disabled | apiforge mcp: resources/read: path_rejected | apiforge mcp: resources/read: path_rejected | apiforge mcp: resources/read: unknown_document
```

### Scenario 1: check a proposal for a registered document (criterion 1)

Tests: `mcp03_criterion_1_check_text_reports_the_proposal_for_a_registered_path`
and `mcp03_criterion_1_a_text_over_one_mebibyte_is_too_large_before_anything_else`
in `crates/apiforge-mcp/tests/acceptance.rs`.

Expected: the diagnostics the text would have as that document, positioned as
`read_source` positions them for the same text saved; `valid`, `generation`,
the saved `source_sha256` and the published revisions used; the files and
`.apiforge/local` unchanged.

Observed: as expected. The test saves the same proposal in a second checkout
and compares `check_text`'s diagnostics with `read_source`'s, for a warning
and for an error. Over stdio, `export ???` in `components/product.afs` gave
an error at 16:10 with `generation: 0`, the saved hash `eba04ff2…` and three
revisions used; the checksums above did not change. A text over 1 MiB is
`too_large` before any other check.

### Scenario 2: check a document that does not exist yet (criterion 2)

Tests: `mcp03_criterion_2_an_unregistered_path_is_checked_as_a_new_document`
and `mcp03_criterion_2_the_errors_a_registration_would_meet`.

Expected: `registered: false`, the kind from the header, the handle it would
get, and the registration problems: path on disk, a case-insensitive
collision, a taken handle, a header without a kind.

Observed: as expected. Over stdio, `components/new-product.afs` with
`component new_product` gave `registered: false`, kind `component`, handle
`new_product`, valid, no problems; the demo's own product text at
`components/copy.afs` gave `handle_exists`; `hello = 1` gave kind `null` and
`invalid_text`. The tests also cover a registered path that differs in case,
an unregistered file on disk and a file differing in case (`path_exists`);
problem messages never quote the path.

### Scenario 3: unpublished revision, unknown handle, missing variable (criterion 3)

Test:
`mcp03_criterion_3_unpublished_revisions_unknown_handles_and_missing_variables_are_named`,
for a registered and an unregistered path.

Observed: passes. Over stdio the errors are *Component revision does not
exist* on `product@2` (2:10), *Unknown operator or nested component call* on
`nosuch@1` (3:11) and *Unknown environment name* on `env.MISSING` (4:23), the
messages the window shows.

### Scenario 4: run the saved scenario (criterion 4)

Tests: `mcp03_criterion_4_run_simulates_the_saved_scenario` and
`mcp03_criterion_4_a_failed_verdict_is_a_result_not_an_error`.

Expected: `passed` with 18 steps on the demo shop, one entry per step with
handle, label, status, message, nested for a component call; `generation`,
`source_sha256`, `redacted: true`. A failed comparison carries expected and
actual and is a result, not a tool error.

Observed: as expected. Over stdio: `passed`, `ended_by: completed`,
`step_count: 18`, `redacted: true`, eight top-level steps with the component
steps nested, the 18 lines `apiforge run` prints. With `expected_price = 81`
the test gets `failed` on `promotional_price` with expected 81 and actual 80.

### Scenario 5: run a component alone (criterion 5)

Test: `mcp03_criterion_5_a_component_runs_alone_with_its_inputs`.

Observed: passes. `components/product.afs` runs with its default `id`, with
`{"id": 7}` and with `{"id": "7"}` (text the dialog would read as a number).
Over stdio, `{"id": 7}` passed with the steps `fetch_product`, `product`;
`components/active-promotion.afs` without inputs was `invalid_inputs` with
`missing: ["product"]`; `{"nope": 1}` was `invalid_inputs` with
`unknown: ["nope"]`. The test also covers `invalid: ["id"]` for
`{"id": "seven"}` and inputs given to a scenario.

### Scenario 6: compact by default, full on request, the 2 MiB bound (criterion 6)

Tests: `mcp03_criterion_6_compact_by_default_and_the_full_report_on_request`
and `mcp03_criterion_6_a_result_over_two_mebibytes_is_refused_never_cut`.

Observed: passes. The compact demo result is 3 448 characters over stdio
(under 10 000, asserted). `full: true` returned the runner's report (`mode:
simulation`, eight top-level steps, the outputs). A scenario whose full report
exceeds 2 MiB is `result_too_large`, with `full: false` in the message and
`details.limit` and `details.bytes`, never cut; its compact result passes.

### Scenario 7: an invalid or unregistered document (criterion 7)

Test: `mcp03_criterion_7_run_refuses_an_invalid_or_unregistered_document`.

Observed: passes. A saved syntax error is `invalid_text` with the
diagnostics; `components/missing.afs` is `unknown_document` with the
registered paths (also over stdio); `../x.afs` is `path_rejected`.

### Scenario 8: live is refused and sends nothing (criterion 8)

Test: `mcp03_criterion_8_live_is_refused_and_sends_nothing`.

Observed: passes. The project's base URL is pointed at a loopback listener
that counts connections; `live: true` is `live_disabled` with *live requests
are not enabled on this server* and `wrote: false`, a simulation then
passes, and the listener accepted 0 connections. Over stdio the same error
came back with the same message.

### Scenario 9: the shared executor returns data (criterion 9)

Tests: `mcp03_criterion_9_a_folder_run_prints_what_it_printed_before_the_shared_executor`
in `crates/apiforge-cli/tests/run.rs` and
`mcp03_criterion_9_the_shared_executor_returns_data_and_prints_nothing` in
the MCP acceptance tests.

The byte-identity test: `crates/apiforge-cli/tests/fixtures/demo-shop-run.txt`
was recorded from `apiforge run` on the demo shop before the executor moved,
and committed with the acceptance tests in `ae1cb14`; its SHA-256 is
`618147c53aebc50c035eefb12fe209291d2611bf0a4dfd806120a343103c4dde`. The test
runs the built `apiforge run --folder <demo> --scenario
scenarios/promotion-and-cart-price.afs` and compares stdout with it, byte for
byte, with exit code 0 and an empty stderr. The second test asserts that
`apiforge-mcp/src/executor.rs` contains no `println!`, `print!(`,
`eprintln!`, `eprint!(`, `stdout` or `stderr`, and that `execute` returns
the report.

Observed: both pass. The release binary's stdout in smoke item 6 below has
the same SHA-256, `618147c5…`. On Windows the test failed in CI run
40590319155: the checkout turned the fixture into CRLF while the command
prints LF. `468d16b` keeps the fixture at LF on every checkout in
`.gitattributes`, as the JSON and `.afs` fixtures already are.

### Scenario 10: cancellation (criterion 10)

Tests: `mcp03_criterion_10_cancelling_a_run_stops_it_and_runs_the_cleanup`
(library) and `mcp03_criterion_10_a_cancelled_run_over_stdio` (binary).

Expected: `notifications/cancelled` ends the run with `status: cancelled`,
the steps done so far and the cleanup run; `project_overview` is answered
meanwhile; a second run is `run_in_progress`.

Observed: as expected, with the change on the response stated below. The
library test gets `cancelled`, the first step `passed`, a later main step
`cancelled`, the two cleanup steps not cancelled, and the slot free. Over
stdio with a 300 ms pause after each step:

```text
-- session cancel; seams {'APIFORGE_MCP_TEST_STEP_DELAY_MS': '300'}
>> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run","arguments":{"path":"scenarios/promotion-and-cart-price.afs"}}}
>> {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"project_overview","arguments":{}}}
<< {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\n  \"name\": \"Demo shop\", …"}], …}}
>> {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"run","arguments":{"path":"components/product.afs"}}}
<< {"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"{\n  \"code\": \"run_in_progress\",\n  \"message\": \"Another run is in progress on this server; wait for its result or cancel it\", …"}], …}}
>> {"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2,"reason":"qa"}}
>> (ids 5–7: run components/product.afs, each answered run_in_progress while the cleanup ran)
>> {"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"run","arguments":{"path":"components/product.afs"}}}
<< {"jsonrpc":"2.0","id":8,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"components/product.afs\",\n  \"status\": \"passed\", …"}], …}}
-- slot free after 1.82 s: run components/product.afs status=passed
-- EOF: exit code 0 after 0.003 s; stdout lines 7, non-protocol lines 0; a response to id 2: False
-- stderr: apiforge mcp: run: run_in_progress (×4) | apiforge mcp: run: cancelled
```

### Scenario 11: the deadline (criterion 11)

Tests: `mcp03_criterion_11_the_deadline_ends_a_run_as_timed_out`,
`mcp03_criterion_11_the_in_flight_request_is_marked_as_possibly_delivered`
and `mcp03_criterion_11_a_run_past_the_deadline_over_stdio`.

Observed: all pass. Over stdio with a 300 ms deadline and a 200 ms pause:

```text
-- session deadline; seams {'APIFORGE_MCP_TEST_STEP_DELAY_MS': '200', 'APIFORGE_MCP_TEST_DEADLINE_MS': '300'}
>> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run","arguments":{"path":"scenarios/promotion-and-cart-price.afs"}}}
<< {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"scenarios/promotion-and-cart-price.afs\",\n  \"status\": \"cancelled\",\n  \"ended_by\": \"timed_out\", …"}], …}}
-- run: status=cancelled ended_by=timed_out steps: product=passed/main, active_promotion=cancelled/main, expected_price=cancelled/main, add_to_cart=cancelled/main, promotional_price=cancelled/main, field_inna_nazwa=cancelled/main, delete_cart=skipped/cleanup, cart_was_deleted=skipped/cleanup
>> {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"project_overview","arguments":{}}}
-- overview after: name=Demo shop
-- EOF: exit code 0 after 0.003 s; stdout lines 3, non-protocol lines 0
-- stderr: apiforge mcp: run: timed_out
```

The cleanup phase was evaluated: `delete_cart` and `cart_was_deleted` are
`skipped` because the cart they delete was never created (`add_to_cart` was
cancelled), not `cancelled`. The in-flight marking is shown against the
executor in live mode with a listener that accepts and never answers:
cancelled by the caller and by the deadline, exactly one step,
`fetch_product`, is `cancelled` with `may_have_reached_api: true`.

### Scenario 12: the end of stdin during a run

Test: `mcp03_eof_during_a_run_cancels_it_and_ends_the_process`.

Observed: the test passes (exit 0 within five seconds, nothing written).
Over stdio, closing stdin 0.4 s into a run with a 300 ms pause gave exit 0
after 2.97 s, and the run was **answered** `passed`: rmcp 3.8.0 drains
in-flight requests for up to five seconds at EOF before it cancels them, and
the paused demo run (about 3 s) ends within that. With a 2 000 ms pause the
run outlasted the drain: exit 0 after 5.64 s, no response, stderr `apiforge
mcp: run: cancelled`. The guide said that a run is cancelled at the end of
stdin; `9468081` states the five-second drain. The process always ends
within the drain plus the cleanup wait.

### Scenario 13: the reference and the resources (criterion 12)

Tests: `mcp03_criterion_12_the_language_reference_and_its_version` and
`mcp03_criterion_12_resources_list_and_read`.

Observed: both pass. `language_reference` returns
`docs/language/reference.md` as embedded, version `1`; `project_overview`
reports `reference_version: 1`. Over stdio, `initialize` announces the
`resources` capability beside `tools`; `resources/list` returns the reference
and the four sources in registration order; `resources/read` returns them as
`text/plain`, the source equal to the file. `%2e%2e/x.afs` and
`.apiforge/diagram.json` are refused with `-32602` and `data.code:
path_rejected`; an unregistered path is `-32002` with `unknown_document`.
The test adds `..%2F`, `%2Eapiforge%2F…`, `//etc`, `CON.afs` and `%ff`.

### Scenario 14: carry-overs from the review of MCP-01

Tests: `mcp03_carry_over_1_project_level_errors_are_kept_within_the_limit`,
`mcp03_carry_over_3_a_source_over_one_mebibyte_is_unavailable_and_the_rest_served`,
`mcp03_carry_over_4_positions_do_not_rescan_a_long_line`,
`mcp03_carry_over_5_errors_are_bounded_as_results_are`, and the unknown-method
assertion of `crates/apiforge-cli/tests/mcp.rs`, which now matches the
request's id only (carry-over 2).

Observed: all pass. The folder-pin error and the kind error are kept when
1 102 diagnostics are found (1 000 kept, 102 counted); a source over 1 MiB is
`unavailable: "too_large"` while the rest is served and the scenario still
runs on the published revision; 200 000 position lookups on a 500 000
character line take under two seconds; an error result over 2 MiB is
`result_too_large`.

### Scenario 15: verification (criterion 13)

At `9468081`:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: no
  warning.
- `cargo test --workspace --all-targets --locked`: **1 192 passed, 0 failed, 30
  ignored** over 59 test binaries, exit status 0.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit
  status 0.

The analysis and the scenario language changed (errors kept within the
diagnostic limit), so the guide's extra checks were run in release:

- `cargo run -p apiforge-project --example analysis --release --locked`:
  parse, lower and validate at 1 000 lines took 1.80 ms (budget 5 ms); 66.06
  ms at 1 MiB; analysis after an edit 4.81 ms at 1 000 lines and 173.31 ms
  at 1 MiB. The machine was shared with another build (load average about
  5 on 3 cores); the hand-off run at `3824480` measured 0.63 ms at 1 000
  lines.
- `fuzz_syntax -- 60 1592639710`: 3 545 339 cases, 4 962 valid, 60 s, no
  failure.
- `fuzz_semantics -- 60 2718281828`: 64 100 cases, 9 721 lowered without
  errors, no failure.

The *MCP server* section of the development guide documents `check_text`
versus `run` in one sentence: *`check_text` checks a proposed text without
saving it, while `run` executes the saved source, so a model that wants to
run its draft writes it first.*

## Changes from the ticket

- **Slow step for criterion 10.** A simulation sends no requests, so a
  listener that never answers cannot slow a simulated step. Cancellation
  through the server uses a test seam instead: a pause after each runner
  snapshot, `RunSettings.step_delay`, or `APIFORGE_MCP_TEST_STEP_DELAY_MS`
  for the binary. The never-answering listener is still used, against the
  executor in live mode, to show that the in-flight step is marked
  `may_have_reached_api` (criterion 11).
- **No request count in the demo API.** `apiforge_demo_api` has no in-memory
  request count, and adding one would change a crate outside the ticket.
  Criterion 8 is tested with a loopback listener that counts every
  connection, set as the project's base URL.
- **Response to a cancelled run.** rmcp sends no response to a request after
  `notifications/cancelled`, as the protocol requires. Over stdio a cancelled
  run therefore shows as stderr `apiforge mcp: run: cancelled` and the run
  slot becoming free (scenario 10). The cancelled report (status `cancelled`,
  the steps run so far, the cleanup) is asserted through the library
  function.
- **New loader.** The tolerant load is a new
  `ProjectSession::open_saved_tolerant`, used only by the MCP tools, so that a
  source over 1 MiB does not stop the server. The window and the CLI keep
  their loaders.

## Windows

From CI run 40594524031 of this branch at `9468081` (windows-2022), the
code of the PR apart from this report:

- `cargo test --workspace --all-targets --locked`: 1 181 passed, 0 failed,
  30 ignored over 59 test binaries. The eleven tests that run on Linux and
  not on Windows are Unix symlink and permission tests of other tickets.
  The passing tests include every MCP-03 test, the stdio sessions of the
  binary among them
  (`mcp03_check_text_run_the_reference_and_resources_over_stdio`,
  `mcp03_criterion_10_a_cancelled_run_over_stdio`,
  `mcp03_criterion_11_a_run_past_the_deadline_over_stdio`,
  `mcp03_eof_during_a_run_cancels_it_and_ends_the_process`) and the
  byte-identity test of criterion 9, which had failed in run 40590319155 at
  `3824480` on the CRLF fixture (scenario 9).
- The folder-project and managed-startup step passed. Of its captures,
  the three at 960 × 540 have the standard size; the three named 1600 × 1000
  are 1024 × 768, because the runner's display is smaller (a warning on the
  run, as the testing guide describes). The Code capture at 960 × 540 shows
  the managed demo shop, in the runner's Polish locale, open with "No
  problems": [windows-code-960x540.png](windows-code-960x540.png). Nothing
  on screen changes in this ticket; the capture shows the window still
  starts.
- The stdio transcripts, the checksums and the smoke list were not run on
  Windows by hand; the owner's Windows run is pending.

## Not verified

- **Claude Code registration of the new tools: not verified by the
  implementing agent; left to the owner.** With the release binary of this
  branch registered as in the guide, `/mcp` should list `check_text`, `run`
  and `language_reference` beside the MCP-01 tools, and
  `@apiforge:apiforge://reference` in a prompt should attach the reference.
- A real ten-minute run: the deadline is tested only through the seam.
- `notifications/cancelled` for a run over stdio shows no report on the wire
  (no response is sent); the cancelled report is checked through the library
  function only.
- Cancellation of a live run through the server: `live` is refused until
  MCP-06, so only the executor is exercised in live mode.
- Protocol revision `2026-07-28` (`server/discover`) for the new tools and
  resources: the sessions above used `2025-11-25`; MCP-01's tests cover the
  discovery handshake.

## Regression smoke list

Linux, 2026-10-27, release binaries of `013ccfe`, on Xvfb (`xvfb-run`, X11
backend, Lavapipe), with empty scratch `XDG_DATA_HOME` and `XDG_CONFIG_HOME`.

| # | Result |
| --- | --- |
| 1 | Pass: `apiforge-desktop` with no arguments created and opened the managed demo shop under the scratch data directory (`apiforge/projects/shop`, `recent-projects.json`). Ended by `timeout` after 15 s. The application wrote nothing to stderr; the only output was Mesa's two `libEGL warning: DRI3 error …` lines from the virtual display. |
| 2 | Pass: `--screenshot … --folder <demo> --mode code --size 960x540` showed the explorer tree and the scenario open in Code with "No problems"; exit 0. Capture: [smoke2-code-960x540.png](smoke2-code-960x540.png). |
| 3–5, 7–10 | Not run: no interface change. The workspace suite that covers them passes. |
| 6 | Pass: `apiforge run --folder <demo> --scenario scenarios/promotion-and-cart-price.afs` printed the 18 steps `ok`, then `Passed`; exit 0, empty stderr; stdout SHA-256 `618147c5…`, the recorded fixture of criterion 9. |
