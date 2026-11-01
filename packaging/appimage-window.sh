#!/bin/bash
# REL-01 (#115), criterion 6: opens the AppImage's window on a fresh Xvfb
# display with Mesa's Lavapipe Vulkan driver and captures it.
#
#   packaging/appimage-window.sh APPIMAGE OUT.png
#
# Xvfb is started directly rather than through xvfb-run, so that a bare system
# needs no xauth. Fails unless the capture exits 0 within 45 s and is not empty.
set -euo pipefail
appimage="$1"
output="$2"

for icd in /usr/share/vulkan/icd.d/lvp_icd*.json; do
  if test -f "$icd"; then
    export VK_DRIVER_FILES="$icd"
    break
  fi
done
test -n "${VK_DRIVER_FILES:-}" || { echo "No Lavapipe driver found" >&2; exit 1; }

display=99
while test -e "/tmp/.X$display-lock"; do
  display=$((display + 1))
done
Xvfb ":$display" -screen 0 1920x1200x24 -nolisten tcp &
server=$!
export XDG_RUNTIME_DIR="$(mktemp -d)"
trap 'kill "$server" 2>/dev/null || true; rm -rf "$XDG_RUNTIME_DIR"' EXIT
for _ in $(seq 50); do
  test -e "/tmp/.X11-unix/X$display" && break
  sleep 0.1
done
test -e "/tmp/.X11-unix/X$display" || { echo "Xvfb :$display did not start within 5 s" >&2; exit 1; }
export DISPLAY=":$display"

mkdir -p "$(dirname "$output")"
timeout --kill-after=5s 45s "$appimage" --screenshot "$output" \
  --project fixtures/workspace.json --select 'Cena po promocji'
test -s "$output"
echo "captured: $output ($(stat -c %s "$output") bytes)"
