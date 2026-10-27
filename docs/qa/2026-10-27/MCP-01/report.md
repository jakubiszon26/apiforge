# MCP-01: `apiforge mcp` answers Claude Code — overview, read, check, print — QA report

- Branch and commit: `feat/mcp-01-read-tools` at `4ad875e`. Acceptance tests
  `08515f1`; implementation `c265872` (script), `c8752c8` (project), `35cf93f`
  (mcp), `533e1bd` (cli); development guide `4ad875e`.
- Built with: `cargo build --release --locked` (release binaries
  `target/release/apiforge` and `apiforge-desktop` of `4ad875e`), plus the
  verification commands of the development guide.
- Mock-up: not applicable; nothing on screen changes. Issue:
  [#60](https://github.com/jakubiszon26/apiforge/issues/60). The escalation
  of 2026-10-27 on #60 decided how criterion 5 is met (option A), the two
  accessors, `project_unreadable` and where the stdio test lives.
- Machine: Linux 7.3 (CachyOS), Rust 1.99.0, rmcp 3.8.0, schemars 1.2.2.

## Linux

Every scenario uses the demo shop prepared with `prepare_project` and then
stripped of `.apiforge/local`, which is what a fresh Git clone holds.

### Scenario 1: both protocol revisions over real stdio (criteria 1, 2)

Tests: `crates/apiforge-cli/tests/mcp.rs`,
`criteria_1_and_2_a_discover_session_over_stdio` and
`criteria_1_and_2_an_initialize_session_over_stdio`. Each test spawns
`apiforge mcp --folder <demo>` and drives stdin and stdout.

Expected: `server/discover` (2026-07-28) and `initialize` (2025-11-25) both
name `apiforge` with its version and the `tools` capability. `tools/list`
returns the four tools in a fixed order, each with a description, input and
output schemas and `readOnlyHint: true`. Stdout holds nothing but JSON-RPC
lines through a malformed line, an unknown method and a tool error. EOF ends
the process with exit code 0 within two seconds.

Observed: as expected; both tests pass. The same sessions were then run
against the release binary by a small Python client. These are the lines
exchanged, cut to about 200 characters each; `>>` was sent, `<<` was
received:

```text
>> {"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"qa","version":"1.0.0"},"io.modelcontextprotocol/clientCapabilities":{}}}}
<< {"jsonrpc":"2.0","id":1,"result":{"resultType":"complete","supportedVersions":["2024-11-05","2025-03-26","2025-06-18","2025-11-25","2026-07-28"],"capabilities":{"tools":{}},"instructions":"API Forge project tools. …","ttlMs":0,"cacheScope":"private","_meta":{"io.modelcontextprotocol/serverInfo":{"name":"apiforge","version":"0.1.0"}}}}
>> {"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{…}}}
<< {"jsonrpc":"2.0","id":2,"result":{"resultType":"complete","ttlMs":0,"cacheScope":"public","tools":[{"name":"project_overview",…},{"name":"read_source",…},{"name":"check",…},{"name":"print",…}]}}
>> {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"_meta":{…},"name":"check","arguments":{}}}
<< {"jsonrpc":"2.0","id":3,"result":{"resultType":"complete","content":[{"type":"text","text":"{\n  \"generation\": 0,\n  \"valid\": true,\n  \"documents\": [ …"}],"structuredContent":{"generation":0,"valid":true,"documents":[…]},"isError":false}}
>> { this is not JSON
>> {"jsonrpc":"2.0","id":4,"method":"no/such/method","params":{"_meta":{…}}}
<< {"jsonrpc":"2.0","id":4,"error":{"code":-32601,"message":"no/such/method"}}
>> {"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"_meta":{…},"name":"read_source","arguments":{"path":"../x.afs"}}}
<< {"jsonrpc":"2.0","id":5,"result":{"resultType":"complete","content":[…],"structuredContent":{"code":"path_rejected","message":"Invalid project-relative path.","wrote":false,"details":{}},"isError":true}}
>> {"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"_meta":{…},"name":"read_source","arguments":{"path":"components/product.afs"}}}
<< {"jsonrpc":"2.0","id":6,"result":{"resultType":"complete","content":[{"type":"text","text":"{\n  \"path\": \"components/product.afs\",\n  \"text\": \"component product \\\"Product\\\" { …"}],"structuredContent":{…},"isError":false}}
-- EOF: exit code 0 after 0.001 s; stdout lines 6, non-protocol lines 0
-- stderr: apiforge mcp: read_source: path_rejected
```

```text
>> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"qa","version":"1.0.0"}}}
<< {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"apiforge","version":"0.1.0"},"instructions":"API Forge project tools. …"}}
>> {"jsonrpc":"2.0","method":"notifications/initialized"}
>> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"project_overview","arguments":{}}}
<< {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\n  \"name\": \"Demo shop\",\n  \"base_url\": \"http://127.0.0.1:4318/demo/shop\", …"}],"structuredContent":{…},"isError":false}}
>> not json at all
>> {"jsonrpc":"2.0","id":3,"method":"no/such/method"}
<< {"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"no/such/method"}}
>> {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"print","arguments":{"path":"components/missing.afs"}}}
<< {"jsonrpc":"2.0","id":4,"result":{"content":[…],"structuredContent":{"code":"unknown_document","message":"No registered document has this path","wrote":false,"details":{"registered_paths":["scenarios/promotion-and-cart-price.afs","components/add-to-cart.afs","components/product.afs","components/active-promotion.afs"]}},"isError":true}}
>> {"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"print","arguments":{"path":"components/product.afs"}}}
<< {"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"{\n  \"path\": \"components/product.afs\",\n  \"text\": \"component product …"}],"structuredContent":{…},"isError":false}}
-- EOF: exit code 0 after 0.001 s; stdout lines 5, non-protocol lines 0
-- stderr: apiforge mcp: print: unknown_document
```

The server ignores a malformed line without answering, which is rmcp's
behaviour; a valid request sent after it is answered. An unknown method gets
`-32601` with its id. `tools/list` returns these tools (title and
`readOnlyHint` from the annotations, the input properties, and the output
properties):

| Tool | Input | Output |
| --- | --- | --- |
| `project_overview` | — | `base_url`, `contracts`, `documents`, `generation`, `holder`, `live_requests`, `name`, `reference_version`, `revisions`, `secret_names`, `server_version`, `variables` |
| `read_source` | `path` | `diagnostics`, `generation`, `omitted`, `path`, `source_sha256`, `text`, `unavailable`, `valid` |
| `check` | `path`? | `documents`, `generation`, `valid` |
| `print` | `path` | `generation`, `path`, `source_sha256`, `text` |

### Scenario 2: the overview of the demo shop (criterion 3)

Tests: `criterion_3_the_overview_describes_the_demo_shop` and
`criterion_3_the_generation_is_the_head_generation_after_a_commit`.

Expected: name, base URL, no variables and no secret names, the contracts
`Product` and `Cart`, four documents with kind, path, handle, base revision
and the SHA-256 of their saved bytes, three published revisions with theirs,
`generation`, `holder: none`, live requests not allowed (`no --allow-live`),
the server version and an empty reference version.

Observed: as expected. A fresh clone has generation 0, because the head file
lives in `.apiforge/local`. In a project that has committed (the one
`prepare_project` leaves behind), `generation` equals `head.json`'s
generation.

### Scenario 3: read a source; positions (criterion 4)

Tests:
- `criterion_4_read_source_returns_the_saved_text_and_its_versions`: the text,
  hash and generation of `components/product.afs`, valid, no diagnostics.
- `criterion_4_positions_count_scalar_values_after_multi_byte_characters`:
  `description "Zażółć gęślą 🦀 jaźń" ?? "ł"` gives the error at line 2. The
  column is the scalar-value count before `??`, plus one; the byte count
  would differ. The end column is one more.
- `criterion_4_a_crlf_source_counts_the_carriage_return_as_part_of_the_break`:
  KTLO-25's mismatch in a CRLF file reports 15:28 to 15:46, as in the LF file.
- `criterion_4_the_position_rule`: the rule on short texts: a `\r` before
  `\n`, a lone `\r`, the end of the text.
- `criterion_4_a_fix_comes_with_its_replacement`: `product(id: 123)` without
  a revision carries the fix `product@1` over `product`.
- `criterion_4_an_unknown_path_names_the_registered_paths`: an unknown path
  is `unknown_document`, with `details.registered_paths` listing the four
  paths.

Observed: all pass.

### Scenario 4: check (criterion 5)

Tests:
- `criterion_5_check_reports_the_injected_warning_with_file_line_and_column`:
  `components/product.afs` with `contract "Product"` and the request's
  `contractId` removed, the `75c55f5` shape. `check` without a path returns
  four documents and exactly one problem: `components/product.afs`, 15:28,
  warning, "The value's type does not match this input". With the path it
  returns that document alone.
- `criterion_5_a_project_with_errors_is_reported_not_refused`: a saved
  syntax error is reported with `valid: false`; the other documents stay
  valid.
- `criterion_5_beyond_a_thousand_diagnostics_the_rest_are_counted`: 1 101
  comments that belong to no statement give 1 000 diagnostics and
  `omitted: 101`, from both `check` and `read_source`.
- In `apiforge-script`, `crates/apiforge-script/tests/acceptance.rs`: the
  parser counts 101 dropped warnings and lowering keeps the count. An error
  that lowering adds to a list already full of warnings replaces the last
  warning and counts it as omitted (`omitted: 1`).

Observed: all pass. Before this ticket the analysis dropped diagnostics
beyond 1 000 without counting them. The count now comes from the analysis
crate, where the cap is applied (decision of 2026-10-27 on #60). Which
diagnostics are kept, and in what order, is unchanged.

### Scenario 5: print (criterion 6)

Tests:
- `criterion_6_print_returns_the_canonical_text`: every demo document's text
  equals `print_folder_document`.
- `criterion_6_print_matches_the_print_command_byte_for_byte` (CLI crate):
  the tool's text equals the stdout of `apiforge print --folder … --file …`,
  byte for byte, for the four documents.
- `criterion_6_print_of_an_invalid_source_is_invalid_text_with_the_diagnostics`:
  a document without a valid definition is `invalid_text` with
  `details.diagnostics`.

Observed: all pass.

### Scenario 6: nothing is written (criterion 7)

Test: `criterion_7_a_session_of_all_four_tools_writes_nothing`. Every file
and directory of the checkout is compared before and after a session of all
four tools, including an unknown path.

Observed: byte-identical, and `.apiforge/local` does not exist. The same was
checked with the release binary: SHA-256 of every file before and after the
two stdio sessions of scenario 1, identical, and `.apiforge` still holds only
`diagram.json` and `revisions`.

### Scenario 7: the current directory and outside a project (criterion 8)

Tests:
- `criterion_8_without_folder_the_server_uses_the_current_directory` (CLI
  crate): `apiforge mcp` started in the demo answers `project_overview` with
  "Demo shop". Started in an empty directory, each of the four tools answers
  `not_a_project`, `tools/list` still answers after them, EOF exits 0, and the
  directory stays empty.
- `criterion_8_outside_a_project_every_tool_answers_not_a_project`: the same
  through the functions.

Observed: all pass.

### Scenario 8: the error contract (criterion 9)

Tests:
- `criterion_9_errors_have_the_contract_shape`: the error object has exactly
  `code`, `details`, `message` and `wrote`, with `wrote: false`. Over stdio,
  scenario 1 shows `isError: true` with that object as `structuredContent`.
- `criterion_9_paths_that_fail_working_source_are_rejected`: `../x.afs`,
  `/etc/x.afs`, `.apiforge/diagram.json`,
  `.apiforge/revisions/get-product/1.afs`, `components/CON.afs` and
  `components/com1.afs` are `path_rejected` from `read_source`, `check` and
  `print`, and no message quotes the argument.
- `criterion_9_an_unreadable_project_is_project_unreadable`: a corrupt
  manifest is `project_unreadable` with the storage error's message.

Observed: all pass. The development guide gains the code list, with
`project_unreadable`.

### Scenario 9: verification (criterion 11)

At `533e1bd` (the last code commit; `4ad875e` adds only the development
guide):

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: no
  warning.
- `cargo test --workspace --all-targets --locked`: **1 134 passed, 0 failed,
  29 ignored** over 58 test binaries, exit status 0.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit
  status 0.

The analysis and the scenario language changed (the count of dropped
diagnostics), so the guide's extra checks for those were also run, in
release:

- `cargo run -p apiforge-project --example analysis --release --locked`:
  parse, lower and validate at 1 000 lines took 1.24 ms (budget 5 ms). At
  1 MiB it took 41.27 ms; analysis after an edit took 2.82 ms at 1 000 lines
  and 83.78 ms at 1 MiB.
- `fuzz_syntax -- 60 1592639710`: 10 034 908 cases, 13 877 valid, 60 s, no
  failure.
- `fuzz_semantics -- 60 2718281828`: 156 067 cases, 23 633 lowered without
  errors, no failure.

## Windows

Not run on Windows: there is no CI run for this branch yet. The Windows
captures are to be fetched from the CI artifacts of this branch. Nothing in
the server is platform-specific beyond stdio. The `path_rejected` test
covers the Windows reserved names on every platform.

## Not verified

- **Criterion 10, the manual Claude Code registration: not verified by the
  implementing agent; left to the owner.** In the demo shop folder, with the
  release binary of this branch:

  ```bash
  claude --version
  claude mcp add apiforge -- <path>/target/release/apiforge mcp
  claude          # then /mcp: expect project_overview, read_source, check, print
  ```

  Then a prompt such as *"Using the apiforge tools, read
  components/product.afs and tell me its first line."* should call
  `mcp__apiforge__read_source` and answer `component product "Product" {`.
  Record the `claude --version` output and the prompt used here.
- `result_too_large`: the code is implemented, but no test reaches 2 MiB;
  the demo's largest result is a few kilobytes.
- `recovery_required` from a read: mapped from the storage error, but no
  test interrupts a transaction.
- `holder: another_process`: not exercised; only `none` is tested.
- A `missing`, `not_utf8` or `too_large` saved source in `read_source`
  (`unavailable`): not tested.
- Revision `2026-07-28` beyond rmcp 3.8.0's own handling: the server relies
  on rmcp to negotiate the protocol version and to check the request
  metadata.

## Regression smoke list

Linux, 2026-10-27, release binaries of `4ad875e`, on the host's display
(no Xvfb on this machine), with empty scratch `XDG_DATA_HOME` and
`XDG_CONFIG_HOME`.

| # | Result |
| --- | --- |
| 1 | Pass: `apiforge-desktop` with no arguments created and opened the managed demo shop under the scratch data directory. Ended by `timeout` after 12 s. Stderr held only Mesa's driver line `WARNING: radv is not a conformant Vulkan implementation, testing use only.`, nothing from the application. |
| 2 | Pass: `--screenshot … --folder <demo> --mode code --size 960x540` showed the explorer tree and the scenario open in Code with "No problems". Capture: [smoke2-code-960x540.png](smoke2-code-960x540.png). |
| 3–5, 8–10 | Not run: no interface change. The workspace suite that covers them passes. |
| 6 | Pass: `apiforge run --folder <demo> --scenario scenarios/promotion-and-cart-price.afs` printed every step `ok`, then `Passed`; exit 0. |
| 7 | Pass, through the CLI only: `apiforge apply` of `components/product.afs` with its own bytes left the source and `.apiforge/diagram.json` byte-identical (SHA-256), and `apiforge check` reopened it with exit 0. Ctrl+S in the window was not pressed; nothing in the window changed. |
