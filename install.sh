#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but not installed" >&2
  echo "Install Rust from https://www.rust-lang.org/tools/install" >&2
  exit 1
fi

cargo install --path . -p lazycompass --locked "$@"
