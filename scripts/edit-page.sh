#!/usr/bin/env bash
set -euo pipefail

port="${1:-4173}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Visual page editor: http://localhost:${port}/editor.html"
echo "Press Ctrl-C to stop."
python3 -m http.server "${port}" --directory "${repo_root}/web"
