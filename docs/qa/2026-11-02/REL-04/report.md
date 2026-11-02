# REL-04: The desktop app on Windows opens no console window — QA report

- Branch and commit: `docs/rel-04-no-console` at `df02a3b`. The Windows
  evidence comes from that commit's
  [push CI run 41494860353](https://github.com/jakubiszon26/apiforge/actions/runs/41494860353)
  (`windows-2022`, artifact `windows-icon`); its pull_request run
  [41494869307](https://github.com/jakubiszon26/apiforge/actions/runs/41494869307)
  passed too. Review round 1 (`2172ce7`) made the bash validation match the
  whole `Valid` line and qualified the guide; its push run
  [41504278463](https://github.com/jakubiszon26/apiforge/actions/runs/41504278463)
  printed the same subsystems, process tree (no child process), log blocks,
  rotation and redirected version. The "before" evidence comes from the acceptance commit
  `a4c3a72`, [run 41486729082](https://github.com/jakubiszon26/apiforge/actions/runs/41486729082).
- Built with: `cargo build --release --locked -p apiforge-desktop -p
  apiforge-cli` on `windows-2022` by CI (MSVC, Rust 1.99.0). Linux: the
  verification commands of the development guide, locally and in CI.
- Ticket: [docs/tickets/REL-04.md](../../../tickets/REL-04.md), owner
  decisions 1 = A and 2 = A (2026-11-02). Analysis: Fable. Implementation:
  Claude Opus 5.5.
- Compared against: the ticket's dialog text (the mock-up); nothing in the
  window changes.

## Acceptance criteria

| # | Criterion | How it is verified | Result |
| --- | --- | --- | --- |
| 1 | Release `apiforge-desktop.exe` is `WindowsGui`; release `apiforge.exe` and debug `apiforge-desktop.exe` are `WindowsCui` | CI step *Show the application icon (Windows)* reads `PEHeader.Subsystem` with `PEReader`; `test_release.py` `ConsoleSubsystem`. Scenario 1 | Pass |
| 2 | Started the way Explorer starts it, the window opens and owns no `conhost.exe` | Same CI step: `Start-Process` without `-NoNewWindow` or redirection, `MainWindowHandle`, then `Win32_Process` children; `NoConsoleWindow`. Scenario 2 | Pass: no child process at all |
| 3 | Every existing Windows probe passes unchanged on the GUI build | The native probes, managed startup (created, migrated, restarted), the folder and six QA captures, the icon step | Pass |
| 4 | Started by a parent with no console, `--check` exits 0 and writes the log; a second start appends; a log of 1 MiB + 1 byte becomes `.log.1` | CI step *Write the desktop log without a console (Windows)*; unit tests in `windows_console.rs` on both platforms; `DesktopLog`. Scenario 3 | Pass |
| 5 | A main-thread panic writes the panic text to the log and shows the dialog; a worker panic writes only | `tests/crash_report.rs` on Windows (helper process, temporary log, dialog replaced by a file); `the_crash_dialog_names_the_log`. Scenario 4 | Pass; the real dialog is not shown on CI (*Not verified*) |
| 6 | CI reads the program's output deterministically | `OutputCapture` in `test_release.py`; the bash validation, the probes' stderr files, the piped reads | Pass in `rust.yml`; `release.yml` by test only |
| 7 | `cmd /c "apiforge-desktop.exe --version > out.txt"` writes the version | The log step. Scenario 5 | Pass. The bare terminal attach is the owner's step |
| 8 | No new crate; `apiforge-cli` untouched; the guide documents it | `NoNewCrate`; the lock gains only the edge `apiforge-desktop → windows-sys 0.61.2`; `cargo … --locked` on Linux | Pass |

The acceptance came in two commits, both before the implementation:

- **`a4c3a72`, the workflows and `test_release.py`.** Three tests failed
  locally: the attribute, the `windows-sys` entry and the guide's paragraph.
  On Windows (run 41486729082), the icon step failed with
  `target/release/apiforge-desktop.exe has PE subsystem WindowsCui, expected
  WindowsGui. apiforge-desktop.exe owns a console: conhost.exe 5700.`, and
  the log step failed with `first: C:\Users\runneradmin\AppData\Roaming\apiforge\logs\apiforge-desktop.log
  was not written.` after the console build, started by `WmiPrvSE.exe`,
  exited 0.
- **`1bfade4`, the Rust tests.** They failed to compile on both platforms
  (run 41487485526): `cannot find function log_path / start_line / rotates /
  open_log / dialog_text / log_to / report_crashes`.

Two acceptance tests changed after the implementation, and the assertions
are named here. The attribute test compares `main.rs` on one line, because
rustfmt wraps the attribute; its assertion is unchanged. The release-smoke
test now expects `… --check … | Write-Output` instead of a bare
assignment. The first implementation run showed that the analysis' premise
was wrong: PowerShell 7 does not wait for a windowed program whose output
is only assigned. It closes the pipe, and the program's `println!` failed
with os error 232. See the ticket's implementation notes.

## Windows

All from run 41494860353 on `windows-2022`, at 1024 × 768.

### Scenario 1: the PE subsystem of each `.exe`

Expected: `WindowsGui` for the release desktop binary, `WindowsCui` for the
other two. Observed, from the step log:

```text
target/release/apiforge-desktop.exe: PE subsystem WindowsGui
target/release/apiforge.exe: PE subsystem WindowsCui
target/debug/apiforge-desktop.exe: PE subsystem WindowsCui
```

### Scenario 2: started like Explorer, no console window

The step starts `apiforge-desktop.exe --folder <the migrated demo shop>`
with `Start-Process -PassThru`, without `-NoNewWindow` and without
redirection. It waits for the window and captures the desktop. Then it
lists the process's parent and children while the window is open.

Expected: the API Forge window and no `conhost.exe` child.

Observed: as expected.

```text
captured: test-results/icon/desktop-window.png (1024 x 768), window 'API Forge'
process tree: pwsh.exe (6112) > apiforge-desktop.exe (6184) > no child process
```

[After](windows-desktop-no-console.png): the window, with only the runner's
own `hosted-compute-agent` console behind it.
[Before](windows-desktop-console-before.png), the acceptance run on the
console build: a second console window titled
`D:\a\apiforge\apiforge\target\release\apiforge-desktop.exe` sits behind the
window. That is the console this ticket removes, and the step reported
`pwsh.exe (3896) > apiforge-desktop.exe (5616) > conhost.exe (5700)`.

### Scenario 3: the log when there is no console

`Win32_Process.Create` starts `apiforge-desktop.exe --check
fixtures/workspace.json` suspended, from the WMI host. The step resumes it
and waits for exit code 0, three times. The data directory is the one the
folder step owned.

Expected: one block, then two; then, after a 1 048 577-byte file was put in
place, that file as `.log.1` and a fresh log of one block. Observed:

```text
first: apiforge-desktop.exe (7484) started by WmiPrvSE.exe (352)
  --- apiforge-desktop 0.1.0 2026-11-02 18:17:34 +00:00 ---
  Valid v2 document: 4 project(s)
second: apiforge-desktop.exe (5680) started by WmiPrvSE.exe (352)
  --- apiforge-desktop 0.1.0 2026-11-02 18:17:34 +00:00 ---
  Valid v2 document: 4 project(s)
  --- apiforge-desktop 0.1.0 2026-11-02 18:17:34 +00:00 ---
  Valid v2 document: 4 project(s)
rotated: apiforge-desktop.exe (4416) started by WmiPrvSE.exe (352)
rotated: C:\Users\runneradmin\AppData\Roaming\apiforge\logs\apiforge-desktop.log.1 holds the 1048577 bytes of the previous log
rotated: C:\Users\runneradmin\AppData\Roaming\apiforge\logs\apiforge-desktop.log
  --- apiforge-desktop 0.1.0 2026-11-02 18:17:35 +00:00 ---
  Valid v2 document: 4 project(s)
```

The icon step's window (Scenario 2) wrote no log. Its parent, `pwsh`, has
a console, so the window attached to it, which is decision 2 = A.

### Scenario 4: a crash

`a_panic_on_the_window_thread_is_logged_and_shown` and
`a_panic_on_a_worker_thread_is_logged_only` passed in the Windows test
step. The helper process redirects its output to a temporary log with the
production `log_to` and installs the production `report_crashes`, with the
dialog replaced by a file. A panic on the installing thread exits 101,
writes `panicked at` and the message after the start line, and the
"dialog" receives exactly *API Forge stopped unexpectedly. What happened is
written to <the log>.* A panic on a spawned thread writes the text, the
process exits 0, and no dialog is shown.

### Scenario 5: output redirected by `cmd`

Observed: `cmd /c "apiforge-desktop.exe --version > file" wrote
'apiforge-desktop 0.1.0'`. The bash step's `--check` read `Valid v2
document: 4 project(s)` on both platforms.

## Linux

Nothing changes: the subsystem attribute is `windows`-gated, and Linux
compiles only `prepare() {}` and, in tests, the platform-neutral parts of
the log. Locally and in CI: `cargo fmt --check`, clippy with `-D warnings`,
the workspace tests (the seven `windows_console` unit tests included),
`convert-v1 --fixtures --check` and `test_release.py` pass. The lock adds
no package.

## Not verified

- **The crash dialog itself.** `rfd::MessageDialog::show()` is the one line
  CI cannot exercise: the test replaces it with a file. Its text is
  unit-tested; nobody has seen the dialog. Nor has anyone seen how it
  behaves over a frozen window when the panic comes from inside the event
  loop.
- **A start from Explorer, the Start menu or a double-click in the ZIP** on
  a real desktop. CI's stand-ins are `Start-Process` without `-NoNewWindow`
  (Scenario 2) and the WMI host (Scenario 3).
- **The attach from a terminal** (decision 2 = A): a bare
  `apiforge-desktop.exe --version` typed in `cmd`, PowerShell or Windows
  Terminal prints into a console no CI step can read. Scenario 2 shows
  indirectly that the window attached to `pwsh`'s console, since no log
  was written. This is the owner's QA step.
- **The release workflow.** The ZIP smoke's subsystem check and its piped
  `--check` run only on a pushed tag. `test_release.py` covers them, and
  the same reads passed in `rust.yml`.
- **Windows 10 and 11 desktops, and other PowerShell versions.** Only
  Windows Server 2022 with PowerShell 7 was used.

## Regression smoke list

2026-11-02, from CI run 41494860353. The change is the Windows subsystem,
a module that runs before `main` parses its arguments, and workflow steps.

| # | Flow | Linux | Windows |
| --- | --- | --- | --- |
| 1 | Start with no arguments | Pass in CI (managed startup creates one project) | Pass in CI, same step, on the GUI build |
| 2 | Open a folder project | Pass in CI (folder captures) | Pass in CI; Scenario 2 opens the shop in Code |
| 3–8 | Library, wiring, editing, simulation, save, dirty close and kill | Not run by hand: untouched; the workspace tests pass | Workspace tests pass in CI |
| 9 | Migrate a legacy workspace | Pass in CI (four projects, original byte-identical) | Pass in CI |
| 10 | Window at 960 × 540 | Pass in CI (QA captures) | Pass in CI (QA captures) |
