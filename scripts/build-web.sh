#!/usr/bin/env bash
set -euo pipefail

dist_dir="${1:-dist}"

cargo build --locked --profile wasm-release --target wasm32-unknown-unknown

mkdir -p "${dist_dir}/assets/cursors"
wasm-bindgen \
  target/wasm32-unknown-unknown/wasm-release/bayesian-visualizer.wasm \
  --out-dir "${dist_dir}" \
  --out-name bayesian_visualizer \
  --target web \
  --no-typescript

cp web/index.html "${dist_dir}/index.html"
cp assets/cursors/finish_link.png "${dist_dir}/assets/cursors/finish_link.png"
cp assets/cursors/shift_held.png "${dist_dir}/assets/cursors/shift_held.png"
touch "${dist_dir}/.nojekyll"
