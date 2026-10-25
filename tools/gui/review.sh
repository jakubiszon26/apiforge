#!/usr/bin/env bash
# Renders the design review views (ignored tests named review_*) and writes a
# gallery. Prints the gallery path. Captures stay in the target directory.
#   tools/gui/review.sh [TEST_FILTER]
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
filter=${1:-review_}
captures=${APIFORGE_CAPTURE_DIR:-${CARGO_TARGET_DIR:-$repo/target}/gui-captures}
state=$(mktemp -d "${TMPDIR:-/tmp}/apiforge-gui-review.XXXXXX")
trap 'rm -rf "$state"' EXIT
[[ "$filter" == review_ ]] && rm -rf "$captures"
mkdir -p "$captures" "$state/config" "$state/data"
export APIFORGE_CAPTURE_DIR="$captures" XDG_CONFIG_HOME="$state/config" XDG_DATA_HOME="$state/data"
cd "$repo"
cargo test -q -p apiforge-desktop --bin apiforge-desktop --locked -- --ignored "$filter"

{
  cat <<'HTML'
<!doctype html>
<html lang="en">
<meta charset="utf-8">
<title>Design review</title>
<style>
  body { margin: 24px; font: 14px system-ui, sans-serif; background: #f4f4f5; color: #18181b; }
  figure { margin: 0 0 32px; }
  img { max-width: 100%; border: 1px solid #d4d4d8; background: #fff; }
  figcaption { margin: 6px 0; font-family: ui-monospace, monospace; }
</style>
<h1>Design review</h1>
HTML
  for image in "$captures"/*.png; do
    name=$(basename "$image")
    printf '<figure><figcaption>%s</figcaption><img src="%s" alt="%s"></figure>\n' \
      "$name" "$name" "$name"
  done
} >"$captures/index.html"
echo "$captures/index.html"
