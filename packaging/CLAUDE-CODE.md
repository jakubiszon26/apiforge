# Using API Forge from Claude Code

`apiforge mcp` serves one API Forge folder project to a model over the Model
Context Protocol on stdio. Register the `apiforge` binary of this archive in
the project folder (use the full path of the extracted directory):

```bash
claude mcp add apiforge -- /path/to/apiforge mcp
```

On Windows the binary is `apiforge.exe`:

```powershell
claude mcp add apiforge -- C:\path\to\apiforge.exe mcp
```

Or check in a `.mcp.json` at the project root instead:

```json
{
  "mcpServers": {
    "apiforge": { "command": "/path/to/apiforge", "args": ["mcp"] }
  }
}
```

Without `--folder` the server serves the directory Claude Code starts it in.
Claude Code lists the tools as `mcp__apiforge__<tool>` in `/mcp`.

The model cannot allow live requests: add `--allow-live` after `mcp` (in the
`claude mcp add` command or in `"args"`) only when it may send real requests
to the project's base URL. Secrets come from `APIFORGE_SECRET_<NAME>` in the
server's environment (the `env` of the entry in `.mcp.json`) or from a
`--secrets secrets.json` file, never from the model.

On Linux the command-line binary comes from the `.tar.gz` archive. The AppImage
holds only the window, `apiforge-desktop`, and cannot be registered as the MCP
server.

`apiforge --version` and `apiforge-desktop --version` print the version of
each binary.
