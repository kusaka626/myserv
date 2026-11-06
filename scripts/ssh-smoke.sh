#!/usr/bin/env bash
# Disposable local SSH test. Never point this script at a real server.
set -euo pipefail
cd "$(dirname "$0")/.."
docker build --secret "id=proxy_ca,src=${CODEX_PROXY_CERT:-/etc/ssl/certs/ca-certificates.crt}" \
  -f infra/dev/ssh.Dockerfile -t myserv-ssh-test infra/dev
name="myserv-ssh-test-$$"
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
docker run -d --name "$name" -p 127.0.0.1::22 myserv-ssh-test
printf '%s\n' "root:test-only-\$'\\!" "ubuntu:test-only-\$'\\!" "limited:test-only-\$'\\!" | docker exec -i "$name" chpasswd
MYSERV_TEST_SSH_PORT=$(docker port "$name" 22/tcp | sed 's/.*://')
export MYSERV_TEST_SSH_PORT
for attempt in {1..30}; do
  if docker logs "$name" 2>&1 | grep -q 'Server listening'; then break; fi
  sleep 1
done
cargo test -p myserv-core --locked --offline --test ssh openssh_root_and_sudo -- --ignored
