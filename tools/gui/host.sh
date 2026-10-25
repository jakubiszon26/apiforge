#!/usr/bin/env bash
# Starts an egui_inspection host for egui_mcp and prints "HOST READY <address>".
#   tools/gui/host.sh headless          the folder window in a test harness, no display
#   tools/gui/host.sh native [ARGS...]  the application on a private Xvfb display
# Configuration and data are isolated; the host runs until it is stopped.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
mode=${1:-headless}
shift || true
state=$(mktemp -d "${TMPDIR:-/tmp}/apiforge-gui-host.XXXXXX")
cleanup() {
  [[ -n "${xvfb:-}" ]] && kill "$xvfb" 2>/dev/null
  rm -rf "$state"
}
trap cleanup EXIT
mkdir -m 700 "$state/config" "$state/data" "$state/runtime"
export XDG_CONFIG_HOME="$state/config" XDG_DATA_HOME="$state/data"
cd "$repo"

case "$mode" in
  headless)
    # The test prints the ready line with the free port it bound.
    cargo test -q -p apiforge-desktop --bin apiforge-desktop --locked \
      testing::inspection_host -- --ignored --exact --nocapture
    ;;
  native)
    command -v Xvfb >/dev/null || { echo 'native mode needs Xvfb' >&2; exit 1; }
    for icd in /usr/share/vulkan/icd.d/lvp_icd*.json; do
      [[ -f "$icd" ]] && export VK_DRIVER_FILES="$icd" && break
    done
    [[ -n "${VK_DRIVER_FILES:-}" ]] || { echo 'native mode needs mesa-vulkan-drivers (Lavapipe)' >&2; exit 1; }
    cargo build -q -p apiforge-desktop --features inspection --locked
    # Xvfb chooses a free display and reports it.
    Xvfb -displayfd 3 -screen 0 1920x1200x24 -nolisten tcp 3>"$state/display" 2>/dev/null &
    xvfb=$!
    until [[ -s "$state/display" ]]; do sleep 0.05; done
    port=5720
    while (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; do port=$((port + 1)); done
    (( $# > 0 )) || set -- --project fixtures/workspace.json
    echo "HOST READY 127.0.0.1:$port"
    env -u WAYLAND_DISPLAY DISPLAY=":$(cat "$state/display")" WINIT_UNIX_BACKEND=x11 \
      WGPU_BACKEND=vulkan XDG_RUNTIME_DIR="$state/runtime" EGUI_INSPECTION="127.0.0.1:$port" \
      "${CARGO_TARGET_DIR:-$repo/target}/debug/apiforge-desktop" "$@"
    ;;
  *)
    echo "usage: $0 headless | native [application arguments]" >&2
    exit 2
    ;;
esac
