# MCP-09: The live toggle — the person allows or forbids the agent's live requests in the open window

- Size: M · Owner review: 25 min · Risk: `visible UI`, `default`
- State: Backlog · owner's requirement of 2026-10-27 (updates MCP-00
  decision 4 and MCP-06) · MCP-00 decisions 14, 15, 16 pending · waits for
  mock-up M3 · after MCP-08b and MCP-06
- Analysis model: Fable · Implementation model: —
- Analysis: [MCP-00](MCP-00.md#live-consent-the-toggle)

## Goal

The person decides, per project and at any moment, whether the agent may
send real requests: a box in the project settings, reachable while the agent
is working, obeyed by the server at once, impossible for the agent to flip,
and never overridden by the start flag while the window is open.

## What the owner will see

- New: in *Project settings*, a section *Agent* with ☐ *Allow the MCP agent
  to send live requests* and a line under it saying what applies now
  (*Off — the agent can only simulate* / *On — the agent may send real
  requests to <base URL>* / *A live run is in progress; it will finish. No new
  one starts.*). While ticked, the header shows a *Live allowed* badge. The
  dialog opens while observing, with the name and contracts disabled and the
  toggle enabled.
- Changed: `project_overview.live_requests` tells the agent the state and
  its source (`window setting` / `--allow-live` / `no --allow-live` / `off in
  the project's settings`); a refused live run says *live requests are off
  in this project's settings*.
- Removed, hidden, migrated: nothing. Simulations and the window's own *Run
  live requests* are unaffected.
- Default behaviour that changes: off by default; remembered per project and
  person until changed (decision 15 A); while a window is connected it
  decides and `--allow-live` neither enables nor overrides (MCP-00
  *Live consent*).

## Mock-up

M3: *Project settings* with the *Agent* section in observer mode, and the
header with the *Live allowed* badge, at 960 × 540 and 1600 × 1000.

## Acceptance criteria

1. The box is off for a project never set; ticking it and reopening the
   window shows it ticked; another project is unaffected; the setting lives
   in the application's preferences directory keyed by the project's manifest
   id and not in any file under the project folder (a test asserts the folder
   is byte-identical).
2. *Project settings* opens while observing; only the toggle and *Close* are
   enabled.
3. With the agent connected and the box off, `run(live: true)` is
   `live_disabled` with the settings message; ticking the box — no restart
   of the window or the server — makes the next `run(live: true)` send; the
   agent's `project_overview` shows `allowed: true, source: "window setting"`
   within one second of the change.
4. The server started with `--allow-live` and the box off: `run(live: true)`
   is refused while the window is connected; after the window closes, the
   flag applies again.
5. The server started without `--allow-live` and the box on: `run(live:
   true)` sends while the window is connected.
6. No tool or resource changes the setting; the server has no message that
   sets it from its side (a test sends every tool and asserts the file and
   the slot unchanged).
7. Unticking during a live run: the run finishes, cleanup included, the
   dialog shows the in-progress line, the agent's result is complete, and the
   next `run(live: true)` is refused (decision 16 A).
8. The *Live allowed* badge is visible in the header in Code, Diagram and
   Split at 960 × 540 whenever the box is ticked, and absent otherwise.
9. The QA report compares the dialog and badge against M3; the verification
   commands of the development guide pass.

## Not in scope

A per-run prompt; a consent that travels with the project; the window's own
run mode (UX-02); cancelling the run from the window.

## Technical appendix

- Storage: `agent-consent.json` beside `preferences.json` in the user's
  configuration directory (`preferences.rs::default_path`'s parent), version
  1, `{ "<manifest id>": { "live": true, "changedAt": "…" } }`, atomic write,
  best effort as preferences are.
- Dialog: `dialogs::SettingsDialog` gains the section; `open_project_settings`
  opens it in observer mode with `enabled(false)` on the other controls.
- Channel: the window sends `permission { live }` on connection and on every
  change (MCP-08b's channel); the server writes `Permission::Window` into the
  slot of MCP-06; the channel's loss resets it to `Permission::Flag`.
- The badge: `chrome.rs` beside *Observing*/*Read-only*, `theme::WARNING`.
- Tests: `folder/acceptance_tests.rs` with the server spawned; the consent
  file is pointed at the test's temporary directory through
  `XDG_CONFIG_HOME` as `tools/gui/review.sh` does.
