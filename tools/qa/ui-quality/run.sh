#!/usr/bin/env bash
# Runs P6 UI acceptance probes in an isolated export; red until the requirements pass.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
ref=${1:-HEAD}
qa_dir=$(mktemp -d /tmp/apiforge-ui-quality.XXXXXX)
mkdir -p "$qa_dir/source" "$qa_dir/config" "$qa_dir/data"
git -C "$repo" archive "$ref" | tar -x -C "$qa_dir/source"
cat "$repo/tools/qa/ui-quality/probes.rs" >> "$qa_dir/source/crates/apiforge-desktop/src/folder/tests.rs"
export XDG_CONFIG_HOME="$qa_dir/config" XDG_DATA_HOME="$qa_dir/data"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target}"
printf 'Source: %s\nEvidence: %s\n' "$(git -C "$repo" rev-parse "$ref")" "$qa_dir"
cd "$qa_dir/source"
cargo test -p apiforge-desktop --bin apiforge-desktop --locked --offline ui_quality -- --test-threads=1 --nocapture 2>&1 | tee "$qa_dir/results.log"
