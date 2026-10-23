# Native window checks in CI

The Rust workflow keeps the compatibility-fixture check, formatting, clippy,
workspace tests, release build and `--check` validation on Linux and Windows.
It additionally launches the release application's real native window twice on
each platform:

- `--smoke-test` draws three frames and exits successfully only after the probe
  completes. Loading or renderer startup errors produce a nonzero exit code.
- `--screenshot` renders the fixture workspace, saves a PNG of the application
  viewport and exits. CI requires a successful process exit and a nonempty file.

These commands use a session workspace; they do not change the user's saved
workspace or execute HTTP requests. The Linux screenshot selects the comparison
node so its inspector is also visible.

## Linux

The job installs Xvfb and Mesa's Vulkan software driver. Each probe gets a fresh
1920 × 1200 X11 display through `xvfb-run`. Vulkan is restricted to the explicit
Lavapipe ICD installed by Ubuntu's `mesa-vulkan-drivers` package. A missing ICD
fails the check rather than silently selecting a different backend. A private
`XDG_RUNTIME_DIR` is removed when the step exits.

GNU `timeout` limits each probe to 45 seconds and kills it after another five
seconds if graceful termination fails. Bash runs with `set -euo pipefail`, so
application failures and timeouts fail the step.

## Windows

The job requests the DX12 backend and opens a native window on the hosted runner.
PowerShell starts each probe as a process, waits for at most 45 seconds, terminates
the process tree on timeout and checks the actual exit code. Renderer startup
failures fail the job; there is no fallback to a headless validation command.

Both platform steps also have a three-minute workflow timeout as a final bound.
Captures are uploaded as `native-window-<runner OS>` artifacts, including after
later steps fail. Missing captures do not override an earlier failure; the probe
step itself enforces that its expected capture exists and is nonempty.

## Scope and current evidence

This change configures the checks; it does not establish that GitHub Actions has
executed them successfully. Native Linux captures have previously been generated
locally, but that does not prove the hosted Xvfb/Lavapipe setup or Windows runner
passes. The first remote run must verify those environments.

The probes detect startup, frame rendering, capture and process-exit failures.
They are not pixel-diff tests or end-to-end editing tests, and do not establish
Windows accessibility, installer behavior, signing or production release
acceptance. Inspect the uploaded captures when reviewing layout changes.
