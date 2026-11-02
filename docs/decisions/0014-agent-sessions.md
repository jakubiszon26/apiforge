# ADR 0014: Agent sessions — hand-over of the writer role and live consent

- Status: accepted 2026-10-27 (MCP-00 revision 2, decisions 10–16 = A);
  implementation pending (MCP-04, MCP-07, MCP-08a–c, MCP-09)
- Date: 2026-10-27
- Scope: `apiforge-storage` (holder description), `apiforge-project`,
  `apiforge-mcp`, desktop
- Related: [transactions](0011-project-transactions.md),
  [sessions](0012-project-sessions.md), [run journal](0009-run-journal.md)

## Context

ADR 0011 gives a folder project one writer, chosen by an OS lock that is
never taken over, and lets any number of readers follow the committed state.
An MCP server that works for a model is a second kind of writer, and the
owner wants to keep the window open and watch the model's committed changes
as they land. Two readings of ADR 0011 were possible: the server takes the
lock per write and the window takes it back whenever it is free, which lets
the window cut the model off after its first save; or the server holds the
lock for its lifetime, which makes every window opened meanwhile read-only
with no way back. Neither says how the window learns what the model did,
nor how the person's unsaved work is protected, nor who may allow live
requests.

## Decision

1. **The writer role is held for an agent session.** The server takes the
   writer lock when the model first needs to write and keeps it until the
   model calls `finish_editing`, the client disconnects, or the process
   ends. The length of a pause between tool calls never ends a session.
2. **A holder is never dispossessed by time.** The window becomes the writer
   only when the lock is free: released by its holder, or the holder is
   dead and the operating system released it. ADR 0011's rule — never
   unlink, replace or force-take-over a held lock — stands unchanged.
3. **The holder names itself.** After taking the lock the holder writes one
   JSON line to `.apiforge/local/holder.json`, replaced atomically: program,
   role (`window`, `agent`, `command`), start time, the client's name when
   known, and the endpoint of its control channel when it has one. Never a
   path, never a secret. The lock file itself stays empty and is never
   unlinked: on Windows the OS lock is mandatory and a held lock file cannot
   be read. A reader trusts the holder file only while the probe says a
   writer is live; a holder file without a live writer is stale and ignored.
   An older build that wrote none gets the generic wording.
4. **Hand-over is a request over a control channel**, not a file protocol:
   the lock holder listens on a same-user local endpoint (Unix socket under
   the runtime directory, named pipe on Windows); the other side connects.
   Messages: `request`, `granted`, `refused`, `saved`, `run`, `released`,
   `permission`, `take_back`, newline-delimited JSON with a version. A
   window hands the project over only from a consistent state: nothing
   unsaved in sources, inspector fields, configuration, layout or held
   recovery drafts. It never saves, discards or declares work saved on the
   person's behalf; with unsaved work it refuses or asks the person.
5. **While it is not the writer, the window is an observer.** Editing is
   paused; navigation is free; every commit by the holder is followed by
   reloading the project from one stable read (ADR 0011), so the window
   shows a whole committed generation — source, checkpoint identities and
   positions, registry, revisions — or the previous one, never a mix. A
   reload starts a new epoch (ADR 0012) so a late analysis result cannot
   reinstate an older state. The window keeps the person's place across the
   reload where it is still valid.
6. **Live consent is the person's, in the window, and current.** Live runs
   through the server are allowed when the connected window's per-project
   setting says so; with no window connected, when the server was started
   with `--allow-live`. The setting is remembered per project and person
   outside the project folder, default off, and no tool can change it. A
   change applies to the next live run; a run in progress finishes with its
   cleanup, because interrupting a request leaves effects the journal can
   only call uncertain (ADR 0009).
7. **Writes name the version they were based on.** A read returns the source
   hash and the project generation; a write names the hash it expects and
   is refused when the disk differs. The lock does not replace this check:
   the model's picture of the project may be older than its lock.
8. **A new document is validated before its first byte is written**, in the
   project's context, and its source, registry entry and checkpoint commit in
   one transaction. ADR 0011's boundaries apply: before activation nothing;
   after activation recoverable intent that the next writer finishes.

## Consequences

- The window has three states toward a project: writer, observer, and the
  read-only of a holder that offers no channel (today's). The observer is
  new on screen and a `default` change; pausing edits in it removes an
  in-memory edit that could never be saved.
- The server has one execution path with and without a window; the window
  is never a dependency of the model's work, only of the person's view.
- Both processes implement a small listener; Windows named pipes are a
  verification cost the first shipping ticket records.
- `.apiforge/local/holder.json` is new; it is untracked, so the project
  file format is unchanged.
- Exactly-once execution of external requests is not promised; a lost
  response has a way to learn the outcome (`request_id`, revision hashes,
  the journal) and nothing is retried on the server's own initiative.

## MCP-12 amendment: descriptive overview and client discovery (2026-10-28)

Approved decisions D1–D3 and verification are in [MCP-12](../tickets/MCP-12.md).
The overview retains a string `holder` (`none`, `window`, `another_process`,
`agent_session`), adds nullable `holder_details` (role/client/since) and
`hands_over_on_write`. Only this server's active session is `agent_session`;
a valid window endpoint permits asking without guaranteeing hand-over.
Overview does not connect or interpret another holder's journal. The owner
accepted the prerelease closed-enum extension; tools/list exposes its schema
without repurposing the crate, protocol, language or local format versions.

Listeners keep their existing locations and endpoints remain random names;
only a relative temporary directory, which no client searches, is now skipped
by the listener as well (review of 2026-11-01), so it falls back to `/tmp`.
Unix clients, including the window observing a server, discover existing
sockets in order: absolute XDG_RUNTIME_DIR, `/run/user/<uid>`, temp_dir(),
`/tmp`. Candidates are deduplicated and bounded by the Unix path limit. The uid
comes from metadata of the same opened holder file that supplied its bytes;
it remains in memory, never in the version-one JSON. Missing metadata fails
conservatively. An endpoint directory must be a real directory, and its
socket a real socket, both owned by that uid with no group/world access;
symlinks and other node types are skipped. The shared `/tmp` base is allowed
because the endpoint directory provides privacy. Discovery creates, removes
and changes nothing. It selects the first valid socket; a failed connection
does not retry another endpoint or replay a hand-over request. Filesystem
checks narrow ordinary races, not malicious same-user activity. Windows
retains its named pipe and existing access rules.

A fixed candidate list cannot discover an arbitrary custom XDG or TMPDIR
when its value is absent from the client. Same-side Windows/WSL execution is
required. After a failed connection to a live window, Linux projects under
`/mnt/<single ASCII letter>/…` receive a qualified same-side hint in the
existing `held_by_window`, `wrote: false` refusal. Reads remain available;
there is no close-window advice. A path and failed connection do not establish
a Windows writer, and invisible cross-OS locks remain unsupported. Stale
metadata never prevents acquiring a free OS lock. No project-format, channel
message, live-consent or ownership rule changes.

## MCP-09 amendment: when a window's consent is heard (2026-11-01)

Decision 6 stands; [MCP-09](../tickets/MCP-09.md) fixes when the server
knows the window's consent. A window says `permission { live }` on every
control-channel connection as soon as it opens, before anything else, and on
every change; it never takes a `permission` from the other side. While any
window is connected to the server the windows decide: the newest consent
heard among them, and off while a connected window has not said one yet;
only when the last one goes does the start flag decide again. A live run
that finds the project held by a window naming an endpoint hears that
window's consent before it asks for the project: a window whose consent is
off, or that says nothing within two seconds, is not asked, keeps the
project, and the run is `live_disabled` (*live requests are off in this
project's settings*), whatever the flag. A window of a build older than
MCP-09 says nothing, so it never allows a live run; the window and the
server ship together. The overview still never connects, so before a window
is connected it reports the flag. The server
tells connected windows `run { mode: live }` when its live run starts and
ends. The consent is kept in `agent-consent.json` beside the preferences, by
the manifest id; the project file format is unchanged.

## MCP-15 amendment: a command asks the window too (2026-11-01)

Decisions 2–5 stand; [MCP-15](../tickets/MCP-15.md) adds a second kind of
asker. `apiforge run --folder … --live`, the person's own command, asks a
window that holds the project and names its endpoint exactly as the server
does: `request` gains an optional `role` (`agent`, `command`; absent is an
agent, so an older peer reads it unchanged and `version` stays 1). The
window hands over or asks the person as for an agent, observes while the
command holds the project, and offers no *Take back* for a command, whose
live run is never interrupted. The command waits for the person's answer
without the server's thirty-second limit, tells the window `run` when its
run starts and ends, and says `released` after letting the lock go at every
end; a killed command is taken back by the window's check (decision 2). The
person's `--live` is the consent: decision 6's per-project setting governs
the agent only, and a `permission` from the window is ignored by the
command. A command never asks an agent session (decision 1) and does not
listen; `apply` keeps refusing. The project file format is unchanged.

## MCP-16 amendment: the observer follows the holder (2026-11-01)

Decision 5's last sentence keeps the camera and the selected blocks in
every case; the active document and the code's scroll stay only while
following is unpinned. [MCP-16](../tickets/MCP-16.md) records the owner's
decisions 1–7. An
observing window follows every holder's commits by default: the reload's
diff (`ExternalOutcome::followed`) names what changed, and the window goes
to the document of the commit's main change (created, saved, published,
laid out, in that order) in the view the person is in, shows a saved
source's first changed line and marks its changed lines in the gutter
until the holder's next commit to that document. A deleted document's
tab closes as before, and when it was the active one the window stays on
the tab closing chose; a commit of the settings alone and a run move
nothing. The person's own
navigation to another document unpins following, as does *Follow* in the
bottom bar, which pins it again and goes to the last change. Following
starts pinned with every observation and is never persisted. The changed
lines are computed from the two committed texts in `apiforge-project`
(`CommitChange::lines`); no channel message, protocol version, file format
or persisted setting changes.
