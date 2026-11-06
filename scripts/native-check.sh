#!/usr/bin/env bash
# Linux host with Rust installed through rustup and access to the local Docker daemon.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=$(pwd)
docker build -f infra/dev/Dockerfile -t myserv-native-check infra/dev
# Use a separate target directory because container and host libc versions may differ.
docker run --rm --user "$(id -u):$(id -g)" \
  -v "$repo:$repo" -v "$HOME/.cargo:/rust/cargo" -v "$HOME/.rustup:/rust/rustup:ro" \
  -e CARGO_HOME=/rust/cargo -e RUSTUP_HOME=/rust/rustup \
  -e CARGO_TARGET_DIR="$repo/target/tauri-linux" -w "$repo" \
  myserv-native-check /rust/cargo/bin/cargo check --offline --locked --manifest-path apps/client/src-tauri/Cargo.toml -j 4
