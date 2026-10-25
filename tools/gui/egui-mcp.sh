#!/usr/bin/env bash
# Runs the pinned egui_mcp server on stdio (registered in .mcp.json), or installs it
# with --install. The release must match the egui minor version used by eframe.
set -euo pipefail
version=0.2.0
root="${XDG_DATA_HOME:-$HOME/.local/share}/apiforge/tools/egui_mcp-$version"
binary="$root/bin/egui-mcp"

if [[ "${1:-}" == "--install" ]]; then
  cargo install egui_mcp --version "=$version" --locked --root "$root"
  printf 'Installed %s\n' "$binary"
  exit 0
fi
if [[ ! -x "$binary" ]]; then
  printf 'egui_mcp %s is not installed. Run: tools/gui/egui-mcp.sh --install\n' "$version" >&2
  exit 1
fi
exec "$binary" "$@"
