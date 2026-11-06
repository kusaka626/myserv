#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash -n infra/preflight.sh infra/install.sh
sh -n infra/awg/entrypoint.sh
(cd apps/client && npm ci && npm test && npm run build)
