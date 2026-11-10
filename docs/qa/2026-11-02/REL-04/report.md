# REL-04: The desktop app on Windows opens no console window — QA report

- Branch and commit: `docs/rel-04-no-console` at `1be6577`, after the
  owner's ruling of 2026-11-02 (decision 2 = B, no unsafe code). The
  Windows evidence comes from that commit's
  [push CI run 41527482901](https://github.com/jakubiszon26/apiforge/actions/runs/41527482901)
  (`windows-2022`, artifact `windows-icon`). The "before" evidence comes
  from the acceptance commit `a4c3a72`,
  [run 41486729082](https://github.com/jakubiszon26/apiforge/actions/runs/41486729082).
- Built with: `cargo build --release --locked -p apiforge-desktop -p
  apiforge-cli` on `windows-2022` by CI (MSVC, Rust 1.99.0). Linux: the
  verification commands of the development guide, locally and in CI.
- Ticket: [docs/tickets/REL-04.md](../../../tickets/REL-04.md). Owner
  decisions: 1 = A, and 2 = B (first A, changed on PR #206). Analysis:
  Fable. Implementation: Claude Opus 5.5.
- Compared against: the ticket's dialog text (the mock-up); nothing in the
  window changes.

## The owner's decision change

The first implementation (`e680929` to `108b640`) built decision 2 = A.
It attached to the parent console with `AttachConsole`, sent the standard
handles to the log with `SetStdHandle`, and relaxed the crate's lints to
allow those two `unsafe` calls. The owner ruled out unsafe code and chose
B. `1be6577` removes all of it, and the manifests and `Cargo.lock` are
again as on `main`. Every line the program prints now goes through one
function. That function writes to the stream the parent gave, or, on
Windows, to the log when the parent gave none. The check is the standard
library's safe `AsRawHandle`, which is null for a stream that was not
given.

Acceptance assertions changed because of this decision:

- `NoNewCrate`: "`windows-sys` is a Windows-only dependency at a locked
  version" became "no `windows-sys`, the workspace's lints with
  `unsafe_code = "forbid"`, no unsafe code in the crate". Also new: "the
  program prints only through `windows_console::write`".
- `DesktopLog`: new, "`cmd` without a redirection writes the version to the
  log" (criterion 7 under B).
- The guide-paragraph test now looks for decision B's wording.
- `crash_report.rs` installs the routing as if no handle were given, where
  it used to redirect the handles to the file. Its assertions are the same,
  plus two printed lines in the log.

Earlier changes not caused by the decision: the attribute test compares
`main.rs` on one line, because rustfmt wraps the attribute. The ZIP-smoke
test expects `… --check … | Write-Output`. PowerShell 7 does not wait for
a windowed program whose output is only assigned; it closes the pipe, and
the program then failed with os error 232 (see the ticket's implementation
notes).

## Acceptance criteria

| # | Criterion | How it is verified | Result |
| --- | --- | --- | --- |
| 1 | Release `apiforge-desktop.exe` is `WindowsGui`; release `apiforge.exe` and debug `apiforge-desktop.exe` are `WindowsCui` | CI step *Show the application icon (Windows)* reads `PEHeader.Subsystem` with `PEReader`; `ConsoleSubsystem`. Scenario 1 | Pass |
| 2 | Started the way Explorer starts it, the window opens and owns no `conhost.exe` | Same CI step; `NoConsoleWindow`. Scenario 2 | Pass: no child process at all |
| 3 | Every existing Windows probe passes unchanged on the GUI build | The native probes, managed startup, the folder and six QA captures, the icon step | Pass |
| 4 | Started by a parent with no console, `--check` exits 0 and writes the log; a second start appends; 1 MiB + 1 byte becomes `.log.1` | CI step *Write the desktop log without a console (Windows)*; unit tests in `windows_console.rs` (both platforms); `DesktopLog`. Scenario 3 | Pass |
| 5 | A main-thread panic writes the panic text to the log and shows the dialog; a worker panic writes only | `tests/crash_report.rs` on Windows; `the_crash_dialog_names_the_log`. Scenario 4 | Pass; the real dialog is not shown on CI |
| 6 | CI reads the program's output deterministically | `OutputCapture`; the bash validation, the probes' stderr files, the piped reads | Pass in `rust.yml`; `release.yml` by test only |
| 7 (B) | `cmd /c "… --version > out.txt"` writes the line; `cmd` without a redirection prints nothing in the terminal and writes the line to the log | The log step; `DesktopLog`. Scenario 5 | Pass |
| 8 | No new crate, lock unchanged, unsafe still forbidden, `apiforge-cli` untouched, the guide documents it | `NoNewCrate`; `--locked` builds | Pass |

The acceptance came first, in two commits:

- **`a4c3a72`** failed on the console build (run 41486729082):
  `target/release/apiforge-desktop.exe has PE subsystem WindowsCui, expected
  WindowsGui. apiforge-desktop.exe owns a console: conhost.exe 5700.`, and
  `first: …\apiforge-desktop.log was not written.`
- **`1bfade4`**'s Rust tests failed to compile because the module's
  functions did not exist (run 41487485526).

## Windows

All from run 41527482901 on `windows-2022`, at 1024 × 768.

### Scenario 1: the PE subsystem of each `.exe`

```text
target/release/apiforge-desktop.exe: PE subsystem WindowsGui
target/release/apiforge.exe: PE subsystem WindowsCui
target/debug/apiforge-desktop.exe: PE subsystem WindowsCui
```

### Scenario 2: started like Explorer, no console window

The step starts `apiforge-desktop.exe --folder <the migrated demo shop>`
with `Start-Process -PassThru`, without `-NoNewWindow` and without
redirection. It waits for the window, captures the desktop, and lists the
process tree while the window is open.

Expected: the API Forge window and no `conhost.exe` child. Observed: as
expected.

```text
captured: test-results/icon/desktop-window.png (1024 x 768), window 'API Forge'
process tree: pwsh.exe (1452) > apiforge-desktop.exe (1768) > no child process
```

[After](windows-desktop-no-console.png): the window, with only the runner's
own `hosted-compute-agent` console behind it.
[Before](windows-desktop-console-before.png), the acceptance run on the
console build: a console window titled
`D:\a\apiforge\apiforge\target\release\apiforge-desktop.exe` sits behind
the window (`pwsh.exe (3896) > apiforge-desktop.exe (5616) > conhost.exe
(5700)`).

### Scenario 3: the log when there is no console

`Win32_Process.Create` starts `apiforge-desktop.exe --check
fixtures/workspace.json` suspended, from the WMI host. The step resumes it
and waits for exit code 0, three times.

```text
first: apiforge-desktop.exe (4812) started by WmiPrvSE.exe (1984)
  --- apiforge-desktop 0.1.0 2026-11-02 21:50:27 +00:00 ---
  Valid v2 document: 4 project(s)
second: apiforge-desktop.exe (5216) started by WmiPrvSE.exe (1984)
  --- apiforge-desktop 0.1.0 2026-11-02 21:50:27 +00:00 ---
  Valid v2 document: 4 project(s)
  --- apiforge-desktop 0.1.0 2026-11-02 21:50:28 +00:00 ---
  Valid v2 document: 4 project(s)
rotated: apiforge-desktop.exe (4656) started by WmiPrvSE.exe (1984)
rotated: C:\Users\runneradmin\AppData\Roaming\apiforge\logs\apiforge-desktop.log.1 holds the 1048577 bytes of the previous log
  --- apiforge-desktop 0.1.0 2026-11-02 21:50:28 +00:00 ---
  Valid v2 document: 4 project(s)
```

### Scenario 4: a crash

`a_panic_on_the_window_thread_is_logged_and_shown` and
`a_panic_on_a_worker_thread_is_logged_only` passed in the Windows test
step. The helper process installs the production routing on a temporary
log, as if the parent had given no handle, with the dialog replaced by a
file. It then prints one line on each stream and panics.

- A panic on the installing thread: the process exits 101. The log holds
  the start line, the two printed lines, `thread '…' panicked at …` and the
  message. The "dialog" receives exactly *API Forge stopped unexpectedly.
  What happened is written to <the log>.*
- A panic on a spawned thread: the text is logged, the process exits 0, and
  no dialog is shown.

### Scenario 5: a start from `cmd` (decision 2 = B)

```text
cmd /c "apiforge-desktop.exe --version > file" wrote 'apiforge-desktop 0.1.0'
cmd /c "apiforge-desktop.exe --version": C:\Users\runneradmin\AppData\Roaming\apiforge\logs\apiforge-desktop.log
  --- apiforge-desktop 0.1.0 2026-11-02 21:50:28 +00:00 ---
  Valid v2 document: 4 project(s)
  --- apiforge-desktop 0.1.0 2026-11-02 21:50:28 +00:00 ---
  apiforge-desktop 0.1.0
```

The second `cmd` ran in a console of its own (`Start-Process cmd.exe … -Wait`),
as a terminal does, without a redirection. The line went to the log as a
new block. The bash step's `--check` read `Valid v2 document: 4 project(s)`
on both platforms.

## Linux

Nothing changes for the person using it. The subsystem attribute is
`windows`-gated, and on Linux `windows_console::write` always prints to
the stream, as `println!`/`eprintln!` did, except that a failed write is now
ignored instead of panicking. Locally and in CI, all of these pass: `cargo fmt --check`,
clippy with `-D warnings`, the workspace tests (the seven `windows_console`
unit tests included), `convert-v1 --fixtures --check` and `test_release.py`.
The debug binary prints `--version` and `--check` as before, and exits 2
with `Unknown argument; use --help` on stderr.

## Not verified

- **The crash dialog itself.** `rfd::MessageDialog::show()` is replaced by
  a file in the test; its text is unit-tested and nobody has seen it. Nor
  has anyone seen how it behaves over a frozen window when the panic comes
  from inside the event loop.
- **A start from Explorer, the Start menu or a double-click in the ZIP** on
  a real desktop. CI's stand-ins are `Start-Process` without `-NoNewWindow`
  and the WMI host.
- **A real terminal.** CI shows that a `cmd` in its own console without
  redirection sends the line to the log. A person typing
  `apiforge-desktop.exe --version` in `cmd`, PowerShell or Windows
  Terminal, seeing nothing printed and finding it in the log, is the
  owner's QA step.
- **A handle that is present but unusable.** The safe check cannot tell it
  from a good one. What would be written to it is lost, as before this
  ticket.
- **The release workflow.** The ZIP smoke's subsystem check and piped
  `--check` run only on a pushed tag. `test_release.py` covers them, and
  the same reads passed in `rust.yml`.
- **Windows 10 and 11 desktops, and other PowerShell versions.**

## Regression smoke list

2026-11-02, from CI run 41527482901. The change is the Windows subsystem,
the routing of the program's printed lines, a panic hook on Windows, and
workflow steps.

| # | Flow | Linux | Windows |
| --- | --- | --- | --- |
| 1 | Start with no arguments | Pass in CI (managed startup creates one project) | Pass in CI, same step, on the GUI build |
| 2 | Open a folder project | Pass in CI (folder captures) | Pass in CI; Scenario 2 opens the shop in Code |
| 3–8 | Library, wiring, editing, simulation, save, dirty close and kill | Not run by hand: untouched; the workspace tests pass | Workspace tests pass in CI |
| 9 | Migrate a legacy workspace | Pass in CI (four projects, original byte-identical) | Pass in CI |
| 10 | Window at 960 × 540 | Pass in CI (QA captures) | Pass in CI (QA captures) |
