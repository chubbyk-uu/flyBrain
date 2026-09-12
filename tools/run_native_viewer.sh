#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_dir"

cleanup() {
  kill "${viewer_pid:-}" "${server_pid:-}" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

target/release/flybrain-world web-view "$@" &
viewer_pid=$!
npm --prefix web start &
server_pid=$!

echo "Open http://localhost:8080/native-view.html (gallery: /native-gallery.html)"
wait "$viewer_pid"
