#!/usr/bin/env bash
set -euo pipefail
fail() { printf 'MYSERV_ERROR:%s\n' "$1"; exit 1; }
trap 'fail checks' ERR
[ "$(id -u)" = 0 ] || fail root
[ -d /run/systemd/system ] || fail systemd
[ -r /etc/os-release ] || fail os
. /etc/os-release
[ "$ID" = ubuntu ] && [ "$VERSION_ID" = 24.04 ] || fail os
case "$(uname -m)" in x86_64|aarch64) ;; *) fail arch;; esac
awk '/MemTotal/ { if ($2 < 1900000) exit 1 }' /proc/meminfo || fail memory
if [ ! -f /var/lib/myserv/owned ]; then
    [ "$(df -Pk /var | awk 'NR==2 {print $4}')" -ge 8000000 ] || fail disk
fi
command -v python3 >/dev/null || fail python
if [ ! -f /var/lib/myserv/owned ]; then
    if command -v k3s >/dev/null || command -v kubeadm >/dev/null; then fail kubernetes; fi
    ports=$(ss -Hlnut)
    if printf '%s\n' "$ports" | awk '{print $5}' | grep -Eq ':(443|51820)$'; then fail ports; fi
    # Do not add privileged networking to a machine already hosting containers.
    if command -v docker >/dev/null; then
        containers=$(docker ps -aq)
        [ -z "$containers" ] || fail containers
    fi
fi
printf 'Сервер подходит для установки\n'
