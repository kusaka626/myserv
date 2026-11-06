#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo run --locked -p myserv-core --example protocol_fixture -- "$work"
cp "$work/xray-server" "$work/server.json"
cp "$work/xray-client" "$work/client.json"
# Optional proxy CA mount for the managed environment. TLS verification stays enabled.
build_args=()
if [ -n "${CODEX_PROXY_CERT:-}" ]; then build_args+=(--secret id=proxy_ca,src=/etc/ssl/certs/ca-certificates.crt); fi
docker build "${build_args[@]}" -t myserv-awg:3.1.20260828 infra/awg
python3 scripts/awg-smoke.py "$work"
for config in server client; do
 docker run --rm --user 0 -v "$work:/configs:ro" ghcr.io/xtls/xray-core:26.10.31 run -test -config "/configs/$config.json"
done
