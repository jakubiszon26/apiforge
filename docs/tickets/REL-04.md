# REL-04: The desktop app on Windows opens no console window

- Size: S · Owner review: 15 min · Risk: `visible UI` (Windows only: the
  console window behind the app goes; a crash shows a message box)
- State: Analysis approved (issue #205) · decisions 1 = A and 2 = A, the
  owner's decisions of 2026-11-02 (quiz on this analysis, recorded on #205)
  · before `v0.1.0` (the owner, 2026-11-02, on the REL-02 acceptance quiz of
  #199)
- Analysis model: Fable · Implementation model: Claude Opus 5.5
- After REL-02 (#199, merged): builds on its *Show the application icon
  (Windows)* step and `packaging/test_release.py`

## Goal

A person who starts API Forge on Windows — from Explorer, the Start menu or
the release ZIP — gets the window and nothing else: no black console window
behind it. Whatever the program would have written to that console is still
somewhere the person can find, and a crash tells them where. The `apiforge`
command line stays a console program.

## What the owner will see

- New: on Windows, `apiforge-desktop.exe` started from Explorer opens only
  the API Forge window. A new file,
  `%APPDATA%\apiforge\logs\apiforge-desktop.log`, next to the managed
  projects of `%APPDATA%\apiforge`, receives everything the program writes
  when it has no console: the control-channel diagnostics (`apiforge:
  channel: …`), an error that ends the start, and the text of a crash. Each
  start appends a line `--- apiforge-desktop <version> <date time> ---`
  before its output; when the file is over 1 MiB at start it is renamed to
  `apiforge-desktop.log.1` first, so there is never more than about 2 MiB.
- New (decision 1 = A): when the window dies of a programming error, a
  standard error dialog *API Forge* says *API Forge stopped unexpectedly.
  What happened is written to %APPDATA%\apiforge\logs\apiforge-desktop.log.*
  with one *OK* button, and the process ends as it does today. Until now
  the window simply vanished.
- Changed (decision 2 = A): from a terminal, `apiforge-desktop.exe
  --version`, `--help`, `--check FILE` and error messages still print there.
  What changes is what Windows does with every windowed program: `cmd` does
  not wait for it, so its prompt may come back before the line, and
  `%ERRORLEVEL%` is only set when the shell waits (`start /wait`, or
  PowerShell's `$x = & …`, which waits because it reads the output). The
  development guide says so in one paragraph.
- Changed: nothing on Linux; nothing in the window; `apiforge.exe` is a
  console program as before. A debug build (`cargo run`) keeps its console
  so that development on Windows is unchanged.
- Removed or hidden: the console window behind the app, which is the goal.
  Migrated, default behaviour that changes: nothing else. The project file
  format and the MCP output are untouched.

## Mock-up

Not applicable: nothing in the window changes. The only new element is the
system's standard error dialog (`rfd::MessageDialog`, level *Error*, title
*API Forge*, the sentence quoted above, *OK*); the text is the mock-up and
the owner approves or rewords it with decision 1.

## How output travels today, and after

Today `apiforge-desktop.exe` is a console-subsystem program. Windows attaches
it to the console of the shell that started it, or creates one when there is
none — the window behind the app after a double-click. The program has no
logging: the desktop crate depends on neither `log`, `env_logger` nor
`tracing`; what eframe, winit and wgpu log goes to no logger and is dropped.
What it writes is `println!` for `--help`, `--version` and `--check`, and
`eprintln!` for an argument or document error, a window that cannot be
initialised, a capture that failed, the migration notes of a managed start
(`main.rs`), and twelve control-channel diagnostics in `folder/handover.rs`.
A panic is printed by Rust's default hook to stderr, and the process exits
with code 101.

After `#![cfg_attr(all(windows, not(debug_assertions), not(test)),
windows_subsystem = "windows")]`, a release build has no console of its own.
Started from Explorer, its standard handles are null: the standard library
treats a missing handle as a sink, so `println!` and `eprintln!` succeed and
write nothing, and a panic's message is lost with them (the exit code stays
101). Started with its output redirected — a pipe, `> file`, PowerShell
capturing into a variable, `Start-Process -RedirectStandardError`, or a
process the Rust standard library spawns — the parent hands the program its
handles explicitly and the output still arrives. Started from a terminal
without redirection, nothing arrives unless the program attaches itself to
the parent's console (decision 2). This ticket adds one Windows-only module
that runs first in `main`: if the program already has standard handles, it
does nothing; else it attaches to the parent console (A) or, when there is
none, opens the log file and installs it as stdout and stderr with
`SetStdHandle`, which the standard library honours because it asks for the
handle on every write; the default panic hook then writes into the log
without any change, and a hook wrapped around it shows the dialog of
decision 1.

## Decisions for the owner

1. **What a crash shows when there is no console** — the window dies of a
   programming error after a start from Explorer or the ZIP.
   - **A (recommended): a dialog that points at the log.** The panic's
     message and location are in `apiforge-desktop.log`, and a standard
     error dialog says *API Forge stopped unexpectedly. What happened is
     written to <path>.* with *OK*; then the process ends as today. A panic
     in a worker thread (storage, control channel) is logged only, because
     the window survives it and reports it as it does now. The person learns
     that the window did not close by itself and where to look; the cost is
     one call of `rfd`, already a dependency.
   - **B: the log only.** The window vanishes; the person finds the log if
     they know it exists. The smallest change.
   - **C: the dialog shows the panic text itself**, message and `file:line`,
     and nothing is written. It tells the person what, in a developer's
     words, and the channel diagnostics before the crash are still lost.

2. **A start from a terminal** — `cmd`, PowerShell, Windows Terminal.
   - **A (recommended): attach to the terminal.** `--version`, `--help`,
     `--check` and error messages print in the terminal as today, and
     nothing from a terminal start goes to the log. What stays is Windows'
     treatment of every windowed program: `cmd` returns its prompt without
     waiting, so the line may appear after the prompt, and `%ERRORLEVEL%` is
     set only under `start /wait`; PowerShell waits whenever it reads the
     output. One console API call; the development guide notes the quirk.
   - **B: do not attach.** From a terminal the program prints nothing unless
     its output is redirected; otherwise the output goes to the log file as
     after an Explorer start. No console API call at all, but
     `apiforge-desktop --version` typed in a terminal prints nothing, which
     looks broken.

### Decided

The owner, 2026-11-02, on #205:

1. **A.** A crash with no console writes the panic text to
   `%APPDATA%\apiforge\logs\apiforge-desktop.log`, and a panic on the main
   thread shows the standard error dialog *API Forge stopped unexpectedly.
   What happened is written to <path>.* with *OK*. A panic on a worker
   thread is logged only.
2. **A.** A start from a terminal attaches to the parent console
   (`AttachConsole(ATTACH_PARENT_PROCESS)`), so `--version`, `--help`,
   `--check` and errors print there; the guide documents that `cmd` does not
   wait for a windowed program.

## Acceptance criteria

Criteria 1–6 run on the `windows-2022` job of `rust.yml` on every push, and
1, 3 and 6 again in `release.yml`'s *Smoke the binaries from the extracted
ZIP*. The workflow steps are themselves checked by tests in
`packaging/test_release.py`, as REL-02's are (`IconWorkflows`).

1. **Subsystem.** The release `apiforge-desktop.exe` has PE subsystem
   `WindowsGui` (2) and the release `apiforge.exe` keeps `WindowsCui` (3);
   the debug `target/debug/apiforge-desktop.exe` that `cargo test` builds
   keeps `WindowsCui`. Test: the *Show the application icon (Windows)* step
   reads `PEHeaders.PEHeader.Subsystem` of each file with
   `System.Reflection.PortableExecutable.PEReader` and fails on any other
   value; the ZIP smoke does the same for the two extracted `.exe` files.
2. **No console window.** Started the way Explorer starts it — `Start-Process`
   without `-NoNewWindow` and without redirection — with `--folder <shop>`,
   the release build opens the API Forge window (`MainWindowHandle` set
   within 45 s, as REL-02 waits for it) and owns **no console**: `Get-CimInstance
   Win32_Process -Filter "Name='conhost.exe' AND ParentProcessId=<pid>"` is
   empty while the window is open (a console program started this way has
   exactly one). The desktop is captured as in REL-02 and the capture joins
   the `windows-icon` artifact. QA step: the report cites the capture; the
   console window of REL-02's capture belonged to the runner, not to the app,
   so the artifact's evidence of this ticket is the conhost assertion.
3. **The window still starts and is observed.** Every existing Windows step
   that starts the desktop binary — `Invoke-NativeProbe` (`--smoke-test`,
   `--screenshot`), `Invoke-Managed` (`--managed`), the folder and QA
   captures, the icon step's window, and the ZIP smoke's `--version` and
   `--check` — passes with its assertions unchanged (exit code 0, the PNGs
   non-empty, the files on disk, the version line). Test: those steps, on
   the GUI build.
4. **Output when there is no console.** Started by a parent that has no
   console (`Invoke-CimMethod Win32_Process Create`, whose parent is the WMI
   host) with `--check fixtures/workspace.json`, the program exits 0 and
   `%APPDATA%\apiforge\logs\apiforge-desktop.log` ends with a `--- apiforge-desktop
   <version> …` line followed by `Valid v<n> document: <k> project(s)`; a
   second such run appends a second block; a file of 1 MiB + 1 byte put
   there before a run is found as `apiforge-desktop.log.1`. Test: a step in
   the Windows job, run after `Reset-DataDirectory` has owned the directory
   so that no real log is touched. Unit tests (both platforms) for the pure
   parts: the start line, the rotation rule and the dialog text.
5. **A crash.** Decision 1 = A: a panic on the main thread writes Rust's
   panic text into the log and shows the dialog; a panic on another thread
   writes the text only. Test: a Windows-only integration test in
   `crates/apiforge-desktop/tests/` spawns the test binary's own helper
   (`#[ignore = "launched by crash_report"]`) with the crash reporting
   installed on a temporary log and the dialog disabled, and asserts exit
   code 101 and the panic text in the file; the dialog's text is unit-tested
   and its `show()` is the one line of `rfd` that cannot be exercised on CI
   — listed in the QA report under *Not verified*. With B the test asserts
   the log only; with C it asserts the exit code and that nothing is
   written.
6. **CI reads the program's output deterministically.** PowerShell does not
   wait for a windowed program whose output it does not read, and a windowed
   child started without redirection has no handles for its errors, so:
   `release.yml`'s ZIP smoke captures `--check`'s output into a variable
   and asserts the `Valid v` line and `$LASTEXITCODE -eq 0`; `rust.yml`'s
   *Validate workspace using the native release binary* (bash, both
   platforms) asserts the `Valid v` line; `Invoke-NativeProbe`, the folder
   and QA captures and the icon step redirect the program's stderr to a
   file and print it when the step fails, as `Invoke-Managed` already does.
   `Start-Process … -PassThru` with `WaitForExit` and `ExitCode`, which
   every step uses, is independent of the subsystem and stays. Test:
   `packaging/test_release.py` reads both workflows and checks each of these
   calls.
7. **Terminal start (decision 2 = A).** `cmd /c "apiforge-desktop.exe
   --version > out.txt"` on the Windows job writes `apiforge-desktop
   <version>` to the file, and `$line = & apiforge-desktop.exe --version`
   keeps working in the icon step and the ZIP smoke (criterion 3). The attach
   itself — a bare `apiforge-desktop.exe --version` typed in cmd, whose
   console no CI step can read — is a QA step on the owner's Windows machine
   and is otherwise listed as not verified; with decision B the criterion is
   that the same command writes the line to the log instead.
8. **Nothing else moves.** The verification commands of the development
   guide pass; a Linux build fetches no new crate (`windows-sys` is a
   `[target.'cfg(windows)'.dependencies]` entry at the version already in
   `Cargo.lock`, so the lock gains no package and the third-party notices of
   REL-03 need no change); `apiforge.exe` and `crates/apiforge-cli` are not
   touched; `docs/development.md` *Running* gains the paragraph of decision
   2 and names the log file. Test: `test_release.py` checks the paragraph as
   `IconDocs` checks REL-02's; the Linux job proves the absence of a new
   dependency by building with `--locked` against an unchanged lock.

## Not in scope

A logging framework or a log level for eframe's and wgpu's own records; a
log on Linux (stderr stays what a terminal or the journal shows); a crash
dialog on Linux; a `--log` or `--console` argument; a Windows installer; the
same treatment for `apiforge.exe`, which is a console program by decision;
the `--help` text, which is unchanged.

## Regression potential and what covers it

Low, and all of it on Windows: the probes that start the window keep their
exit-code assertions (criterion 3); the one way a probe could pass wrongly
— PowerShell not waiting for an unread windowed program — is removed by
criterion 6 and guarded by `test_release.py`; debug and test builds keep the
console through `not(debug_assertions)` and `not(test)`, so `cargo run`,
`cargo test` and the release-mode `frame_budgets` run of the guide are
unchanged; Linux compiles no new code because the attribute is `windows`-gated
and the module is `#[cfg(windows)]`.

## Technical appendix

- **Files.** `crates/apiforge-desktop/src/main.rs` (the attribute; a call
  to `windows_console::prepare()` as the first statement of `main`; the
  panic hook), a new `crates/apiforge-desktop/src/windows_console.rs` (about
  80 lines, `#[cfg(windows)]`, with a no-op twin for other targets so the
  call site has no `cfg`), `crates/apiforge-desktop/Cargo.toml` and the
  workspace `Cargo.toml` (`windows-sys` with features `Win32_Foundation`,
  `Win32_System_Console`), `.github/workflows/rust.yml`,
  `.github/workflows/release.yml`, `packaging/test_release.py`,
  `docs/development.md`, a Windows-only integration test for criterion 5.
- **The attribute** makes rustc pass `/SUBSYSTEM:WINDOWS` and the matching
  entry point to `link.exe`; no linker flag and no change to `build.rs` or
  `winresource` (REL-02) is needed. `not(test)` keeps the test harness a
  console program in every profile, including `cargo test --release`.
- **`prepare()`.** `GetStdHandle(STD_ERROR_HANDLE)` non-null → return (the
  parent redirected the output: a pipe, a file, a variable). Else decision
  2 = A: `AttachConsole(ATTACH_PARENT_PROCESS)`; on success
  Windows fills the standard handles with the console's and the standard
  library uses them from the next write. Else (`ERROR_INVALID_HANDLE`: no
  parent console) open `dirs::data_dir()/apiforge/logs/apiforge-desktop.log`
  with `append`, after the rotation of criterion 4, write the start line,
  and `SetStdHandle` it for `STD_OUTPUT_HANDLE` and `STD_ERROR_HANDLE`,
  leaking the handle for the process lifetime. Any failure here — no data
  directory, the file cannot be created — is ignored: the window must start
  regardless, and the writes then go nowhere, as they would without this
  ticket. `dirs::data_dir()` is the same `%APPDATA%` that managed projects
  use (`main.rs::managed_opening`), so `logs/` sits beside `projects/`;
  nothing enumerates that directory, and CI's `Reset-DataDirectory` checks
  only `workspace.json`, `projects.json` and `projects`.
- **The hook.** `std::panic::take_hook()` is wrapped: the previous hook
  prints as always (into the console or the log); then, when the program
  runs on the log (not attached, not redirected) and the panicking thread is
  the main thread, `rfd::MessageDialog::new().set_level(Error).set_title("API
  Forge").set_description(text).show()` — synchronous, no event loop
  needed, usable from a panic hook because `MessageBoxW`/`TaskDialog` run
  their own loop. The storage worker (`folder/storage.rs`) and the runner
  service already `catch_unwind` their work; their panics are logged by the
  previous hook and reported in the window as today, without a dialog.
- **Why `windows-sys`.** The standard library exposes neither
  `AttachConsole` nor `SetStdHandle`; `windows-sys` is already in the lock
  (0.52, 0.59 and 0.61 as transitive dependencies) and is MIT/Apache-2.0,
  so REL-03's notices already list it. `rfd` 0.17.2 is a dependency since
  the file dialogs.
- **Why the handles, not the call sites.** Twelve `eprintln!` in
  `handover.rs` and ten in `main.rs` keep working unchanged, and so does the
  default panic hook; a logging crate would be a larger change with the same
  visible result and is left out of scope.
- **PowerShell and windowed programs.** `& exe` returns at once for a
  windowed program unless PowerShell reads its output; `Start-Process
  -PassThru` plus `WaitForExit` does not depend on the subsystem;
  `-NoNewWindow` is meaningless for a windowed program and harmless. .NET's
  `Process.Start` passes standard handles only when one of them is
  redirected; without that, a windowed child on the runner has no handles
  (decision 2 = B) or attaches to the runner's own console (A), and in both
  cases a probe's error message misses the job log — which is why criterion
  6 redirects stderr in the probes.
- **Order.** Acceptance commit (criteria 1, 2, 4, 6, 7 and 8 as
  `test_release.py` tests and CI steps that fail on the console build; the
  unit tests of 4; the integration test of 5) → implementation → QA report
  with the `windows-icon` capture and the conhost assertion quoted → pull
  request ready.
