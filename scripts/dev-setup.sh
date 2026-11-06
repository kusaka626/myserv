#!/usr/bin/env bash
set -euo pipefail
cd /workspace/myserv
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
if ! command -v rustup >/dev/null; then
  installer=$(mktemp)
  trap 'rm -f "$installer"' EXIT
  curl --fail --location https://sh.rustup.rs -o "$installer"
  sh "$installer" -y --profile minimal --default-toolchain 1.99.1 --no-modify-path
  . "$HOME/.cargo/env"
fi
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo fetch --locked
cargo fetch --locked --manifest-path apps/client/src-tauri/Cargo.toml
cargo build --workspace --locked -j 4
npm --prefix apps/client ci --no-audit --no-fund
npm --prefix apps/client run build
